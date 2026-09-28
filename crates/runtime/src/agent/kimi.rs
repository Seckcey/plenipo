//! Kimi adapter (Moonshot AI's Kimi Code CLI) over ACP (ADR-015, ADR-027).
//!
//! Kimi's one-task mode (`kimi -p`) takes the prompt only as an argument and changes files
//! without asking, so a task runs `kimi acp` and talks ACP through the shared driver
//! ([`super::acp`]): the prompt, like everything else, goes in on stdin, one process per task.
//!
//! Kimi's own tools cannot be switched off, so ADR-027 (Kimi over ACP, with its file reads and
//! writes going through Plenipo) holds it this way:
//! - Plenipo offers Kimi file access, so every file Kimi reads or writes comes to Plenipo, which
//!   carries it out through Guard under the worker's permissions (none for a worker without);
//! - Kimi's own shell is always refused (workers run programs with Plenipo's `run_command`), and
//!   so is every other request but Plenipo's tool server and file changes by a worker that may
//!   change files; a request is Plenipo's tool server's only by the tool's name in its title
//!   (never a shell command, and never from what the model wrote as the call's input), and each
//!   call allowed that way is noted in the activity; no approval ever covers a whole session;
//! - Kimi's mode is `default` (it asks before acting), or `plan` (read-only) for a worker without
//!   permissions, never `auto` or `yolo`: the task stops if Kimi reports another mode;
//! - the model and thinking level are session settings (`session/set_config_option`), set before
//!   the prompt and checked.
//!
//! Billing (ADR-007 §4): the sign-in check (`kimi provider list`) must show the Kimi subscription
//! provider (`managed:kimi-code`, `source=oauth`). Plenipo runs only that provider's models
//! (`kimi-code/…`), names one on every task, and passes no Kimi or Moonshot key variables.

use std::path::PathBuf;

use crate::agent::acp::{AcpTask, AcpTurn};
use crate::agent::adapter::{
    first_line, ProbeOutput, RuntimeAdapter, Stop, TurnParser, TurnRequest, NETWORK_ENV,
};
use crate::agent::discovery::HostEnv;
use crate::agent::dto::{
    AuthState, AuthStatus, Effort, KnownModel, RuntimeCapabilities, TurnOutcome,
};

pub const ID: &str = "kimi";
const LABEL: &str = "Kimi";

/// The provider behind the Kimi subscription (`kimi provider list`).
const SUBSCRIPTION_PROVIDER: &str = "managed:kimi-code";
/// Its models' names start with this; Plenipo runs no others on Kimi.
pub const MODEL_PREFIX: &str = "kimi-code/";
/// Kimi's own default (`kimi provider list`, 0.34.0), named on every task that names no model,
/// so a different default in Kimi's settings is never used.
pub const DEFAULT_MODEL: &str = "kimi-code/k3";

/// K3's thinking levels (`session/new`, 0.34.0), lowest first.
const K3_THINKING: &[Effort] = &[Effort::Low, Effort::High, Effort::Max];
/// K2.7 Code Highspeed offers `on` and `low`; only `low` is one of Plenipo's levels.
const HIGHSPEED_THINKING: &[Effort] = &[Effort::Low];

/// Kimi's modes Plenipo uses: `default` asks before acting; `plan` is read-only.
const MODE_DEFAULT: &str = "default";
const MODE_PLAN: &str = "plan";

#[derive(Debug, Default, Clone, Copy)]
pub struct Kimi;

impl RuntimeAdapter for Kimi {
    fn id(&self) -> &'static str {
        ID
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn provider(&self) -> &'static str {
        "moonshot"
    }

    fn provider_label(&self) -> &'static str {
        "Moonshot AI"
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        RuntimeCapabilities {
            streaming_text: true,
            resume: true,
            cancel: true,
            structured_results: true,
            // Kimi does not say which credential an ACP task uses; the check before each task
            // must show the Kimi subscription.
            billing_checked_per_turn: false,
            tool_posture: "Kimi never uses its own command line. Every file it reads or \
                           changes goes through Plenipo Guard, inside the project folder, and \
                           only with the worker's permissions. A worker with permissions also \
                           gets Plenipo's file, program, and git tools, each checked by Plenipo \
                           Guard; a worker without permissions only answers."
                .into(),
            // `session/set_config_option` "thinking": the levels K3 offers.
            effort_levels: K3_THINKING.to_vec(),
            // The Kimi subscription's models (`session/new` on the owner's machine,
            // 2026-09-26), K3 (Kimi's default) first. K3's thinking levels were listed; the
            // others' were not checked, except K2.7 Code Highspeed's (`on`, `low`).
            known_models: vec![
                KnownModel::new("kimi-code/k3", "K3", K3_THINKING),
                KnownModel::new("kimi-code/k3-256k", "K3-256k", &[]),
                KnownModel::new("kimi-code/kimi-for-coding", "K2.8 Preview", &[]),
                KnownModel::new(
                    "kimi-code/kimi-for-coding-highspeed",
                    "K2.7 Code Highspeed",
                    HIGHSPEED_THINKING,
                ),
            ],
        }
    }

    fn checked_version(&self) -> &'static str {
        "0.34.0"
    }

    fn install_hint(&self) -> &'static str {
        "Install Kimi Code, Moonshot AI's command-line tool, with the official installer (see \
         moonshotai.github.io/kimi-code). On Windows it puts kimi.exe in \
         %USERPROFILE%\\.kimi-code\\bin. Then choose Re-check."
    }

    fn login_hint(&self) -> &'static str {
        "Open a terminal, run: kimi login — and sign in with the Kimi account that has your Kimi \
         subscription. Plenipo never asks for your password or an API key. Then choose Re-check."
    }

    fn executable_name(&self) -> &'static str {
        "kimi"
    }

    fn known_locations(&self, host: &HostEnv) -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(home) = &host.home {
            // The official installer puts it in ~/.kimi-code/bin (a real kimi.exe on Windows).
            if cfg!(windows) {
                out.push(home.join(".kimi-code").join("bin").join("kimi.exe"));
            } else {
                out.push(home.join(".kimi-code").join("bin").join("kimi"));
                out.push(home.join(".local").join("bin").join("kimi"));
            }
        }
        out.extend(host.system_dirs().iter().map(|d| d.join("kimi")));
        out
    }

    fn auth_args(&self) -> Vec<String> {
        vec!["provider".into(), "list".into()]
    }

    fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus {
        parse_auth(out)
    }

    fn passthrough_env(&self) -> Vec<&'static str> {
        // Kimi finds its settings and sign-in in the user's profile folder; no variable of its
        // own is needed, and none that could carry a key is passed.
        NETWORK_ENV.to_vec()
    }

    fn turn_args(&self, _request: &TurnRequest) -> Vec<String> {
        // Everything else (conversation, model, thinking, mode) goes in ACP messages.
        vec!["acp".into()]
    }

    fn parser(&self, request: &TurnRequest) -> Box<dyn TurnParser> {
        let model = request
            .model
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL.to_owned());
        let refusal = (!model.starts_with(MODEL_PREFIX)).then(|| Stop {
            outcome: TurnOutcome::BillingNotAllowed,
            reason: format!(
                "Plenipo runs Kimi only with the Kimi subscription's models ({MODEL_PREFIX}…); \
                 \"{}\" is not one of them.",
                first_line(&model, 80)
            ),
        });
        // A worker without Plenipo's tools reads nothing and changes nothing.
        let mode = if request.tools.is_some() {
            MODE_DEFAULT
        } else {
            MODE_PLAN
        };
        let mut settings = vec![
            ("mode".to_owned(), mode.to_owned()),
            ("model".to_owned(), model.clone()),
        ];
        if let Some(effort) = request.effort {
            settings.push(("thinking".to_owned(), effort.as_str().to_owned()));
        }
        Box::new(AcpTurn::new(AcpTask {
            runtime_label: LABEL,
            session: request.session.clone(),
            working_dir: request.working_dir.clone(),
            tools: request.tools.clone(),
            model: Some(model),
            session_meta: None,
            file_access: true,
            settings,
            allowed_modes: vec![MODE_DEFAULT.into(), MODE_PLAN.into()],
            // `session/load` is the way checked on the real CLI.
            load_to_resume: true,
            title_is_tool_name: true,
            refusal,
        }))
    }
}

/// `kimi provider list` names each provider with its credential source:
/// `managed:kimi-code  type=kimi  models=4  source=oauth` is the Kimi subscription.
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
    if out.timed_out {
        return status(
            AuthState::Unknown,
            None,
            Some("The sign-in check timed out.".into()),
        );
    }
    if out.exit_code != Some(0) {
        let why = first_line(&out.combined(), 160);
        return status(
            AuthState::Unknown,
            None,
            Some(if why.is_empty() {
                "Kimi's sign-in check failed.".into()
            } else {
                format!("Kimi's sign-in check failed: {why}")
            }),
        );
    }
    let provider = out.stdout.lines().find_map(|line| {
        let mut words = line.split_whitespace();
        (words.next() == Some(SUBSCRIPTION_PROVIDER)).then(|| {
            words
                .filter_map(|w| w.split_once('='))
                .find(|(k, _)| *k == "source")
                .map(|(_, v)| v.to_ascii_lowercase())
        })
    });
    match provider {
        Some(Some(source)) if source == "oauth" => {
            status(AuthState::Subscription, Some("Kimi sign-in"), None)
        }
        Some(Some(source))
            if source.contains("key") || source.contains("env") || source.contains("api") =>
        {
            status(
                AuthState::ApiKey,
                Some("API key"),
                Some(
                    "Kimi's Kimi Code provider uses an API key: usage would be billed to it."
                        .into(),
                ),
            )
        }
        Some(source) => {
            let source = source.map_or_else(|| "not given".to_owned(), |s| first_line(&s, 40));
            status(
                AuthState::Unverified,
                None,
                Some(format!(
                    "Kimi reported a sign-in Plenipo does not recognize (source {source})."
                )),
            )
        }
        // No Kimi subscription provider: not signed in (other providers are never used).
        None => status(AuthState::SignedOut, None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::adapter::{Parsed, ProcessEnd, ProviderSession};
    use crate::agent::tools::{FileAccess, ToolServer};
    use crate::agent::AgentEvent;
    use crate::dto::ExecutionState;
    use serde_json::{json, Value};

    const FIXTURES: &str = "tests/fixtures/kimi-0.34.0";
    /// Round 4 of the owner's check: file access offered, a read, a write, a command.
    const ROUND_4: &str = "process-3-client-file-access";
    /// Its conversation.
    const SESSION: &str = "session_3ea4a020-46d0-4ced-9bc1-6378b1b5fa4b";

    fn fixture(path: &str) -> String {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES);
        std::fs::read_to_string(dir.join(path)).unwrap()
    }

    fn probe(stdout: &str) -> ProbeOutput {
        ProbeOutput {
            exit_code: Some(0),
            stdout: stdout.into(),
            ..ProbeOutput::default()
        }
    }

    fn server() -> ToolServer {
        ToolServer {
            name: "plenipo".into(),
            command: "/app/plenipo".into(),
            args: vec!["--plenipo-tools=t".into()],
            config_file: "/app/t.json".into(),
            call_timeout: std::time::Duration::from_secs(9),
            tools: ["read_file", "write_file", "run_command"]
                .map(str::to_owned)
                .to_vec(),
        }
    }

    fn sent(parsed: &Parsed) -> Vec<Value> {
        parsed
            .send
            .iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    /// Kimi's real answers, line `n` of an `acp/*.agent.jsonl` capture.
    fn agent_line(file: &str, n: usize) -> String {
        fixture(&format!("acp/{file}.agent.jsonl"))
            .lines()
            .nth(n)
            .unwrap()
            .to_owned()
    }

    fn answer(id: u64, result: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
    }

    fn end() -> ProcessEnd {
        ProcessEnd {
            state: ExecutionState::Succeeded,
            exit_code: Some(0),
            started: true,
            detail: None,
            duration_ms: Some(1),
        }
    }

    #[test]
    fn sign_in_check_needs_the_kimi_subscription_provider() {
        // Real output from the owner's machine (signed in with `kimi login`).
        let s = Kimi.parse_auth(&probe(&fixture("provider-list.txt")));
        assert_eq!(s.state, AuthState::Subscription);
        assert_eq!(s.method.as_deref(), Some("Kimi sign-in"));

        let key = "managed:kimi-code  type=kimi  models=4  source=api_key\n";
        assert_eq!(Kimi.parse_auth(&probe(key)).state, AuthState::ApiKey);
        let env = "managed:kimi-code  type=kimi  models=4  source=env\n";
        assert_eq!(Kimi.parse_auth(&probe(env)).state, AuthState::ApiKey);
        let odd = "managed:kimi-code  type=kimi  models=4  source=device\n";
        assert_eq!(Kimi.parse_auth(&probe(odd)).state, AuthState::Unverified);
        let none = "managed:kimi-code  type=kimi  models=4\n";
        assert_eq!(Kimi.parse_auth(&probe(none)).state, AuthState::Unverified);
        // Only another provider (for example an API key the owner added): not signed in.
        let other = "moonshot  type=openai  models=2  source=config\n";
        assert_eq!(Kimi.parse_auth(&probe(other)).state, AuthState::SignedOut);
        assert_eq!(Kimi.parse_auth(&probe("")).state, AuthState::SignedOut);
        // A look-alike name is not the subscription provider.
        let fake = "managed:kimi-code-evil  type=kimi  source=oauth\n";
        assert_eq!(Kimi.parse_auth(&probe(fake)).state, AuthState::SignedOut);

        let failed = ProbeOutput {
            exit_code: Some(1),
            stderr: "error: unknown command 'provider'".into(),
            ..ProbeOutput::default()
        };
        let s = Kimi.parse_auth(&failed);
        assert_eq!(s.state, AuthState::Unknown);
        assert!(s.detail.unwrap().contains("unknown command"));
        let timed_out = ProbeOutput {
            timed_out: true,
            ..ProbeOutput::default()
        };
        assert_eq!(Kimi.parse_auth(&timed_out).state, AuthState::Unknown);
    }

    #[test]
    fn version_comes_from_kimi_version() {
        let real = fixture("version.txt");
        assert_eq!(Kimi.parse_version(&probe(&real)).as_deref(), Some("0.34.0"));
        assert_eq!(Kimi.checked_version(), "0.34.0");
    }

    #[test]
    fn a_task_is_one_acp_process_and_the_prompt_never_an_argument() {
        let request = TurnRequest {
            model: Some("kimi-code/k3-256k".into()),
            effort: Some(Effort::High),
            ..TurnRequest::default()
        };
        assert_eq!(Kimi.turn_args(&request), ["acp"]);
        let mut parser = Kimi.parser(&request);
        let opening = parser.open("secret plan 42").unwrap();
        assert!(opening.iter().all(|l| !l.contains("secret plan 42")));
        let init: Value = serde_json::from_str(&opening[0]).unwrap();
        assert_eq!(init["method"], "initialize");
        // File access through Plenipo; never a terminal.
        assert_eq!(
            init["params"]["clientCapabilities"]["fs"]["readTextFile"],
            true
        );
        assert_eq!(
            init["params"]["clientCapabilities"]["fs"]["writeTextFile"],
            true
        );
        assert_eq!(init["params"]["clientCapabilities"]["terminal"], false);
    }

    /// Open a new conversation with Kimi's real answers, then answer each setting as Kimi does.
    fn configured(request: &TurnRequest) -> (Box<dyn TurnParser>, Vec<Value>) {
        let mut parser = Kimi.parser(request);
        parser.open("Say hi");
        let open = sent(&parser.line(&agent_line(ROUND_4, 0), false));
        assert_eq!(open[0]["method"], "session/new");
        let mut p = parser.line(&agent_line(ROUND_4, 1), false);
        let mut settings = Vec::new();
        loop {
            let msg = sent(&p).remove(0);
            if msg["method"] == "session/prompt" {
                settings.push(msg);
                return (parser, settings);
            }
            assert_eq!(msg["method"], "session/set_config_option");
            let (option, value) = (
                msg["params"]["configId"].clone(),
                msg["params"]["value"].clone(),
            );
            settings.push(msg.clone());
            let reply = answer(
                msg["id"].as_u64().unwrap(),
                json!({ "configOptions": [{ "id": option, "currentValue": value }] }),
            );
            p = parser.line(&reply, false);
        }
    }

    #[test]
    fn mode_model_and_thinking_are_set_before_the_prompt() {
        let request = TurnRequest {
            model: Some("kimi-code/kimi-for-coding".into()),
            effort: Some(Effort::Max),
            tools: Some(server()),
            ..TurnRequest::default()
        };
        let (_, sent) = configured(&request);
        let settings: Vec<(String, String)> = sent[..sent.len() - 1]
            .iter()
            .map(|m| {
                (
                    m["params"]["configId"].as_str().unwrap().to_owned(),
                    m["params"]["value"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(
            settings,
            [
                ("mode".into(), "default".into()),
                ("model".into(), "kimi-code/kimi-for-coding".into()),
                ("thinking".into(), "max".into()),
            ]
        );
        assert!(sent.iter().all(|m| m["params"]["sessionId"] == SESSION));

        // No permissions: read-only `plan`, and Kimi's default model named explicitly.
        let (_, sent) = configured(&TurnRequest::default());
        let values: Vec<&str> = sent[..sent.len() - 1]
            .iter()
            .map(|m| m["params"]["value"].as_str().unwrap())
            .collect();
        assert_eq!(values, ["plan", DEFAULT_MODEL]);
    }

    #[test]
    fn only_the_subscription_models_run() {
        let request = TurnRequest {
            model: Some("moonshot/kimi-k2".into()),
            ..TurnRequest::default()
        };
        let mut parser = Kimi.parser(&request);
        parser.open("Say hi");
        let p = parser.line(&agent_line("process-1", 0), false);
        assert!(
            p.close_input && p.send.is_empty(),
            "no conversation is opened"
        );
        let stop = p.stop.unwrap();
        assert_eq!(stop.outcome, TurnOutcome::BillingNotAllowed);
        assert!(stop.reason.contains("moonshot/kimi-k2"), "{}", stop.reason);
        let r = parser.finish(&end());
        assert_eq!(r.outcome, TurnOutcome::BillingNotAllowed);
    }

    #[test]
    fn a_follow_up_loads_the_conversation_the_way_checked() {
        let request = TurnRequest {
            session: ProviderSession::Resume {
                id: "session_0fe194e1-3bd2-4568-be4a-8ad5d6d06c7c".into(),
            },
            ..TurnRequest::default()
        };
        let mut parser = Kimi.parser(&request);
        parser.open("What did I ask first?");
        // Kimi offers `session/resume` too; Plenipo uses `session/load`, checked on the real CLI.
        let open = sent(&parser.line(&agent_line("process-2", 0), false));
        assert_eq!(open[0]["method"], "session/load");
        // The replayed history is not this task's.
        for n in 1..=9 {
            assert_eq!(
                parser.line(&agent_line("process-2", n), false),
                Parsed::none()
            );
        }
        let first = sent(&parser.line(&agent_line("process-2", 10), false));
        assert_eq!(first[0]["method"], "session/set_config_option");
    }

    #[test]
    fn kimi_s_own_shell_is_refused_and_its_file_changes_need_permission() {
        // Kimi's real permission requests (round 4): a write, then an `echo` command.
        let file = ROUND_4;
        for (tools, write_allowed) in [(Some(server()), true), (None, false)] {
            let request = TurnRequest {
                tools: tools.clone(),
                ..TurnRequest::default()
            };
            let (mut parser, _) = configured(&request);
            // Kimi announces its Write (kind "edit"), then asks.
            parser.line(&agent_line(file, 275), false);
            let p = parser.line(&agent_line(file, 287), false);
            let reply = &sent(&p)[0];
            assert_eq!(reply["id"], 8);
            let option = reply["result"]["outcome"]["optionId"].as_str().unwrap();
            assert_eq!(
                option,
                if write_allowed {
                    "approve_once"
                } else {
                    "reject"
                }
            );
            assert_ne!(option, "approve_always", "never for a whole session");

            // Its shell (kind "execute") is always refused, with a note in the activity.
            parser.line(&agent_line(file, 326), false);
            let p = parser.line(&agent_line(file, 335), false);
            assert_eq!(sent(&p)[0]["result"]["outcome"]["optionId"], "reject");
            assert!(matches!(
                &p.events[..],
                [AgentEvent::Notice { text, .. }] if text.contains("run_command")
            ));
        }
    }

    #[test]
    fn a_plenipo_tool_named_bare_or_prefixed_is_allowed_once() {
        let request = TurnRequest {
            tools: Some(server()),
            ..TurnRequest::default()
        };
        let options = json!([
            { "optionId": "approve_once", "kind": "allow_once" },
            { "optionId": "approve_always", "kind": "allow_always" },
            { "optionId": "reject", "kind": "reject_once" }
        ]);
        for (title, allowed) in [
            ("read_file", true),
            ("mcp__plenipo__run_command", true),
            ("plenipo__write_file", true),
            ("delete_path", false), // not offered to this worker
            ("Read_file", false),
            ("Bash", false),
            ("mcp__other__read_file", false),
        ] {
            let (mut parser, _) = configured(&request);
            let p = parser.line(
                &json!({ "jsonrpc": "2.0", "id": 5, "method": "session/request_permission",
                         "params": { "toolCall": { "toolCallId": "c1", "title": title },
                                     "options": options } })
                .to_string(),
                false,
            );
            let chosen = &sent(&p)[0]["result"]["outcome"]["optionId"];
            let want = if allowed { "approve_once" } else { "reject" };
            assert_eq!(chosen, want, "{title}");
        }
    }

    #[test]
    fn file_reads_and_writes_go_to_plenipo() {
        let file = ROUND_4;
        let request = TurnRequest {
            tools: Some(server()),
            ..TurnRequest::default()
        };
        let (mut parser, _) = configured(&request);
        // Kimi's real read of notes.txt, and its AGENTS.md lookup.
        let p = parser.line(&agent_line(file, 15), false);
        assert!(p.send.is_empty(), "answered once Plenipo has it");
        let read = &p.files[0];
        assert_eq!(
            read.access,
            FileAccess::Read {
                path: "C:/Users/<user>/plenipo-cli-checks/kimi-scratch/notes.txt".into(),
                line: None,
                limit: None,
            }
        );
        let p = parser.line(&agent_line(file, 16), false);
        let lookup = p.files[0].id;
        // Answers go back in the order Plenipo has them, to the right request.
        let reply = sent(&parser.file_answered(lookup, Err("Blocked: outside".into())));
        assert_eq!(reply[0]["id"], 1);
        assert_eq!(reply[0]["error"]["message"], "Blocked: outside");
        let reply = sent(&parser.file_answered(read.id, Ok("hello\r\n".into())));
        assert_eq!(reply[0]["id"], 0);
        assert_eq!(reply[0]["result"]["content"], "hello\r\n");
        assert!(parser
            .file_answered(read.id, Ok("again".into()))
            .send
            .is_empty());

        let write = json!({ "jsonrpc": "2.0", "id": 9, "method": "fs/write_text_file",
                            "params": { "sessionId": SESSION, "path": "notes.txt",
                                        "content": "hi\n" } });
        let p = parser.line(&write.to_string(), false);
        assert_eq!(
            p.files[0].access,
            FileAccess::Write {
                path: "notes.txt".into(),
                content: "hi\n".into()
            }
        );
        let reply = sent(&parser.file_answered(p.files[0].id, Ok("Created".into())));
        assert_eq!(reply[0], json!({ "jsonrpc": "2.0", "id": 9, "result": {} }));
    }

    #[test]
    fn without_permissions_every_file_request_is_refused_here() {
        let (mut parser, _) = configured(&TurnRequest::default());
        let file = ROUND_4;
        let p = parser.line(&agent_line(file, 15), false);
        assert!(p.files.is_empty());
        assert_eq!(sent(&p)[0]["error"]["code"], -32000);
        assert_eq!(p.events.len(), 1, "noted once");
        let p = parser.line(&agent_line(file, 16), false);
        assert!(p.events.is_empty());
        assert!(sent(&p)[0]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no permission"));
    }

    #[test]
    fn a_change_that_never_came_to_plenipo_stops_the_task() {
        let file = ROUND_4;
        let request = TurnRequest {
            tools: Some(server()),
            ..TurnRequest::default()
        };
        let (mut parser, _) = configured(&request);
        parser.line(&agent_line(file, 275), false);
        parser.line(&agent_line(file, 287), false);
        let done = json!({ "jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": "s", "update": { "sessionUpdate": "tool_call_update",
            "toolCallId": "2:tool_uK921zBTVYJfFemv7SKElzXi", "status": "completed" } } });
        let p = parser.line(&done.to_string(), false);
        let stop = p.stop.unwrap();
        assert_eq!(stop.outcome, TurnOutcome::Failed);
        assert!(
            stop.reason.contains("did not go through Plenipo"),
            "{}",
            stop.reason
        );

        // The same change written through Plenipo (Guard allowed it, and it was written) is fine;
        // one Plenipo refused still counts as never come.
        let write = json!({ "jsonrpc": "2.0", "id": 9, "method": "fs/write_text_file",
                            "params": { "path": "test4.txt", "content": "hi\n" } });
        for (answer, stops) in [
            (Ok(String::new()), false),
            (Err("Blocked: test4.txt is a blocked file".to_owned()), true),
        ] {
            let (mut parser, _) = configured(&request);
            parser.line(&agent_line(file, 275), false);
            parser.line(&agent_line(file, 287), false);
            let asked = parser.line(&write.to_string(), false);
            parser.file_answered(asked.files[0].id, answer);
            let p = parser.line(&done.to_string(), false);
            assert_eq!(p.stop.is_some(), stops);
        }
    }

    #[test]
    fn auto_or_yolo_mode_stops_the_task() {
        let request = TurnRequest {
            tools: Some(server()),
            ..TurnRequest::default()
        };
        for update in [
            json!({ "sessionUpdate": "current_mode_update", "currentModeId": "yolo" }),
            json!({ "sessionUpdate": "config_option_update",
                    "configOptions": [{ "id": "mode", "category": "mode", "currentValue": "auto" }] }),
        ] {
            let (mut parser, _) = configured(&request);
            let line = json!({ "jsonrpc": "2.0", "method": "session/update",
                               "params": { "sessionId": "s", "update": update } });
            let p = parser.line(&line.to_string(), false);
            assert!(p.stop.is_some(), "{update}");
            assert!(p.close_input);
        }
        // Kimi's switch between its own two allowed modes is fine.
        let (mut parser, _) = configured(&request);
        let line = json!({ "jsonrpc": "2.0", "method": "session/update", "params": {
            "sessionId": "s", "update": { "sessionUpdate": "current_mode_update",
                                          "currentModeId": "plan" } } });
        assert!(parser.line(&line.to_string(), false).stop.is_none());
    }

    #[test]
    fn a_setting_kimi_does_not_take_fails_the_task() {
        let mut parser = Kimi.parser(&TurnRequest::default());
        parser.open("Say hi");
        parser.line(&agent_line("process-1", 0), false);
        let p = parser.line(&agent_line("process-1", 1), false);
        let mode = sent(&p).remove(0);
        // Kimi says it is still in its `default` mode after being asked for `plan`.
        let p = parser.line(
            &answer(
                mode["id"].as_u64().unwrap(),
                json!({ "configOptions": [{ "id": "mode", "currentValue": "default" }] }),
            ),
            false,
        );
        assert!(p.close_input && p.send.is_empty());
        let r = parser.finish(&end());
        assert_eq!(r.outcome, TurnOutcome::Failed);
        assert!(r.summary.contains("mode = plan"), "{}", r.summary);
    }

    #[test]
    fn a_real_task_reports_its_conversation_model_and_answer() {
        let request = TurnRequest {
            model: Some("kimi-code/kimi-for-coding-highspeed".into()),
            ..TurnRequest::default()
        };
        let mut parser = Kimi.parser(&request);
        parser.open("Reply with exactly: hello from acp");
        parser.line(&agent_line("process-1", 0), false);
        let p = parser.line(&agent_line("process-1", 1), false);
        let mode = sent(&p).remove(0);
        let p = parser.line(
            &answer(
                mode["id"].as_u64().unwrap(),
                json!({ "configOptions": [{ "id": "mode", "currentValue": "plan" }] }),
            ),
            false,
        );
        // Kimi's real answer to switching the model (round 4).
        let model = sent(&p).remove(0);
        let real = agent_line(ROUND_4, 365);
        let mut real: Value = serde_json::from_str(&real).unwrap();
        real["id"] = model["id"].clone();
        let p = parser.line(&real.to_string(), false);
        assert_eq!(
            p.events,
            [AgentEvent::SessionStarted {
                provider_session_id: Some("session_0fe194e1-3bd2-4568-be4a-8ad5d6d06c7c".into()),
                model: Some("kimi-code/kimi-for-coding-highspeed".into()),
            }]
        );
        assert_eq!(sent(&p)[0]["method"], "session/prompt");
        // Kimi's real answer stream: thoughts, the text, the stop reason.
        for n in 2..=37 {
            parser.line(&agent_line("process-1", n), false);
        }
        let r = parser.finish(&end());
        assert_eq!(r.outcome, TurnOutcome::Completed);
        assert_eq!(r.text.as_deref(), Some("hello from acp"));
        assert_eq!(r.ignored_lines, 0);
    }

    #[test]
    fn kimi_gets_no_credentials() {
        let passed = Kimi.passthrough_env();
        for name in &passed {
            assert!(NETWORK_ENV.contains(name), "{name}");
        }
        assert!(Kimi.fixed_env().is_empty());
    }

    #[test]
    fn known_models_match_the_ones_kimi_listed() {
        // `session/new`'s model list on the owner's machine.
        let listed: Value = serde_json::from_str(&agent_line("process-1", 1)).unwrap();
        let options = listed["result"]["configOptions"][0]["options"]
            .as_array()
            .unwrap();
        let mut theirs: Vec<(&str, &str)> = options
            .iter()
            .map(|o| (o["value"].as_str().unwrap(), o["name"].as_str().unwrap()))
            .collect();
        let caps = Kimi.capabilities();
        let mut ours: Vec<(&str, &str)> = caps
            .known_models
            .iter()
            .map(|m| (m.name.as_str(), m.label.as_str()))
            .collect();
        theirs.sort_unstable();
        ours.sort_unstable();
        assert_eq!(ours, theirs);
        assert_eq!(caps.known_models[0].name, DEFAULT_MODEL);
        // K3's thinking levels, as listed.
        let thinking: Vec<&str> = listed["result"]["configOptions"][1]["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["value"].as_str().unwrap())
            .collect();
        let k3: Vec<&str> = caps
            .effort_levels_for(Some(DEFAULT_MODEL))
            .iter()
            .map(|e| e.as_str())
            .collect();
        assert_eq!(k3, thinking);
        assert!(caps
            .known_models
            .iter()
            .all(|m| m.name.starts_with(MODEL_PREFIX)));
        assert!(!caps.billing_checked_per_turn);
    }
}
