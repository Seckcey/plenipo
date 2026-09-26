# Adding an AI tool

This is Plenipo's adapter contract, the "provider adapter SDK/contract" of `ROLLOUT_PLAN.md`
Phase 15. It walks through the `RuntimeAdapter` trait in
[`crates/runtime/src/agent/adapter.rs`](../../crates/runtime/src/agent/adapter.rs), using the
two adapters that ship today as worked examples:
[`claude_code.rs`](../../crates/runtime/src/agent/claude_code.rs) (Claude Code) and
[`codex.rs`](../../crates/runtime/src/agent/codex.rs) (Codex).

Three decision records set the rules:

- **ADR-014 (adding AI tools ahead of Phase 15)** covers the bar every tool must pass, one branch
  per tool, and what stays out.
- **ADR-007 (how Plenipo runs Claude Code and Codex)** covers the launch, sign-in, and billing
  rules every adapter follows.
- **ADR-011 (how Plenipo picks each worker's AI model)** covers what the model and effort
  settings are for.

In code, an AI tool is a _runtime_ and its company is a _provider_. On screen they are "AI tool"
and "AI company" ([word list](../design/vocabulary.md)).

The [contract suite](../../crates/runtime/tests/contract.rs) checks everything below that can be
checked without the real CLI. It runs for every adapter in `builtin_adapters()`.

## 0. Before writing code: check the real CLI

Check the bar on the real CLI first, on Windows (the target). Record every output in the tool's
checklist (`docs/phases/ai-tools-<tool>-checklist.md`). These outputs are the evidence for the
bar. They are also what the parser and the fake CLI must reproduce, so keep them word for word
(with account names and keys removed).

| Check                  | Record                                                                                                                                           | Bar item |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | -------- |
| Install                | The official install command, where the executable lands, and whether Windows gets a real `.exe` or only an npm `.cmd` shim.                     | 1        |
| Version                | `<cli> --version` (or its equivalent).                                                                                                           | 5        |
| One task, no questions | The prompt piped on stdin, with the flags for non-interactive, structured output. It must exit by itself.                                        | 1, 2     |
| Output                 | The raw output of that task: every line, including the session ID, the final answer, and token usage.                                            | 2        |
| Resume                 | A second task that resumes the first one's session by ID and remembers it.                                                                       | 5        |
| Sign-in status         | The status command's output when signed in with the subscription, when signed out, and with an API key (or the CLI's documented wording for it). | 3, 4     |
| Errors                 | The exact text of a usage limit and of an expired sign-in (as seen, or from the CLI's docs or source).                                           | 2        |
| Models and effort      | The models the CLI itself offers (its picker or `--help`) and the effort levels each one accepts.                                                | —        |
| Least privilege        | The flags that keep it from writing files or using the network. Without any, stop and ask the owner.                                             | —        |
| Credentials in the env | Every environment variable the CLI reads for keys, tokens, or cloud billing, so the adapter never passes them.                                   | 3        |

If any bar item fails, stop. Write a finding instead of an adapter (see
[Writing a finding](#writing-a-finding)).

## 1. Identity and AI company

```rust
fn id(&self) -> &'static str { "codex" }            // stored in records; never changes once shipped
fn label(&self) -> &'static str { "Codex" }         // shown on screen
fn provider(&self) -> &'static str { "openai" }     // the AI company, stored with each run
fn provider_label(&self) -> &'static str { "OpenAI" }
fn install_hint(&self) -> &'static str { "Install the Codex CLI: npm install -g @openai/codex …" }
fn login_hint(&self) -> &'static str { "Open a terminal, run: codex login — and choose Sign in with ChatGPT. …" }
```

- The `id` is also a handoff address (`codex`, alongside `role:<title>`) and part of the approved
  program's name (`agent.codex`). Use lowercase letters, digits, and dashes, and keep it the same
  forever: sessions and Ledger records point to it.
- IDs, labels, and executable names are unique across AI tools. Each company ID has one company
  name. Two tools may share a company.
- The hints are shown on screen when the tool is missing or signed out, so write them in plain
  words. Give the official commands only, and say that Plenipo never asks for a password.

## 2. Finding the executable and reading its version

```rust
fn executable_name(&self) -> &'static str { "claude" }
fn known_locations(&self, host: &HostEnv) -> Vec<PathBuf> { /* ~/.local/bin/claude(.exe), … */ }
fn resolve(&self, found: &Path) -> Option<PathBuf> { /* default: Windows accepts only .exe */ }
fn version_args(&self) -> Vec<String> { vec!["--version".into()] }            // default
fn parse_version(&self, out: &ProbeOutput) -> Option<String> { /* default: first N.N[.N] */ }
```

- Discovery searches PATH first, then `known_locations`. The UI never supplies a path.
- On Windows only real `.exe` files run. An npm `.cmd` or `.ps1` shim runs through `cmd.exe`,
  whose argument parsing is unsafe. There are two ways to handle that:
  - Codex's `resolve` maps the shim to the native binary inside the npm package
    (`vendored_binary`), as OpenAI's own SDK does.
  - Claude Code keeps the default, and its install hint sends the owner to the native installer.
- The version is shown on the AI tools page, and a failing version probe marks the tool as broken.
  The default `find_version` reads `2.1.283 (Claude Code)` and `codex-cli 0.157.1` alike.

## 3. Sign-in check and billing classification

```rust
fn auth_args(&self) -> Vec<String> { vec!["login".into(), "status".into()] }   // Codex
fn parse_auth(&self, out: &ProbeOutput) -> AuthStatus;
```

Plenipo runs the status command before every turn, with the environment the turn gets.
`parse_auth` sorts the output into one `AuthState`:

| State             | Meaning                                     | Ready?                             |
| ----------------- | ------------------------------------------- | ---------------------------------- |
| `Subscription`    | Signed in with the subscription account     | Yes                                |
| `ApiKey`          | Signed in with an API key (pay-per-use)     | **Never**                          |
| `ThirdPartyCloud` | Routed through a cloud account that bills   | **Never**                          |
| `SignedOut`       | Not signed in                               | No: the login hint is shown        |
| `Unverified`      | Signed in, but the method is not recognized | Only if `billing_checked_per_turn` |
| `Unknown`         | The check failed, timed out, or is missing  | Only if `billing_checked_per_turn` |

- Recognize the subscription positively. Codex's `"chatgpt"` is an example. Anything you cannot
  place is `Unverified` or `Unknown`, never `Subscription`.
- Keep only a short method label (`"ChatGPT sign-in"`, `"Claude subscription (max)"`). Never keep
  an email address, organization, or key fragment:
  - Codex's output can include a masked key. It is classified and dropped.
  - Claude Code filters labels through `label_value`.
- `spawn_error` and `timed_out` probes are `Unknown`, with the reason in `detail`.
- `billing_checked_per_turn: true` is only for a CLI that reports its credential in every turn's
  output. Claude Code does this with `apiKeySource` in its `init` event. The parser must then stop
  a turn whose credential is not the subscription (§6). A tool without that report must confirm
  `Subscription` up front, as Codex does.

## 4. Environment variables

```rust
fn passthrough_env(&self) -> Vec<&'static str> {
    ["CODEX_HOME"].into_iter().chain(NETWORK_ENV.iter().copied()).collect()
}
fn fixed_env(&self) -> Vec<(String, String)> {
    vec![("DISABLE_AUTOUPDATER".into(), "1".into())]            // Claude Code: no update mid-turn
}
```

- The supervisor clears the environment and starts from a short operating-system baseline. The
  CLI gets only that baseline, `fixed_env`, and the `passthrough_env` names that are set.
- Pass through only what the CLI needs to find its own settings and sign-in (its config
  directory) and the proxy and certificate settings in `NETWORK_ENV`.
- **Never** pass API keys, tokens, passwords, or variables that switch billing to a cloud account
  or another endpoint. Examples: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`,
  `GITHUB_TOKEN`, `CLAUDE_CODE_USE_BEDROCK`, `*_BASE_URL`. The contract suite rejects any name
  containing `KEY`, `TOKEN`, `SECRET`, `PASSWORD`, `CREDENTIAL`, `BEDROCK`, `VERTEX`, `FOUNDRY`,
  or `BASE_URL`.

## 5. Turn arguments: model, effort, resume

```rust
fn preassigns_session_id(&self) -> bool { true }       // Claude Code: Plenipo picks the ID
fn turn_args(&self, request: &TurnRequest) -> Vec<String>;
```

`TurnRequest` holds the session (new, optionally with an ID Plenipo chose, or resumed by the
provider's ID), the model, the effort, and whether billing was confirmed. **It has no prompt.**
Plenipo writes the prompt to the CLI's stdin, so it never shows in process lists, is not limited
by Windows command-line length, and cannot be misquoted.

| Piece                 | Claude Code                                                           | Codex                                              |
| --------------------- | --------------------------------------------------------------------- | -------------------------------------------------- |
| One task, JSON output | `-p --output-format stream-json --verbose --include-partial-messages` | `exec --json --skip-git-repo-check`                |
| Least privilege       | `--tools "" --strict-mcp-config` (no tools, no MCP servers)           | `--sandbox read-only`                              |
| Model                 | `--model <name>`                                                      | `--model <name>`                                   |
| Effort                | `--effort <level>`                                                    | `-c model_reasoning_effort=<level>`                |
| New session           | `--session-id <uuid Plenipo chose>`                                   | nothing: the thread ID arrives in `thread.started` |
| Resume                | `--resume <id>`                                                       | `resume <thread id>` (last)                        |

- The model name is already validated (`validate_model`: a name, never a flag or a path). The
  effort level is one the tool lists.
- The contract suite checks every request shape: new and resumed sessions, the default model and
  each known model, the default effort and each level that model takes. The model, effort, and
  session ID must each appear as an argument of its own or as `--flag=value` / `key=value`.
  No argument may carry a credential (`--api-key`, `--token`, `…password…`, and so on).
- Never add flags that skip the CLI's permission checks wholesale ("yolo" modes). Plenipo Guard
  (Phase 7) is the only thing that grants a worker more.
- After Phase 7 merges, `TurnRequest` also carries Plenipo's tool server (`tools`), and
  `turn_env` adds variables per turn. Follow Phase 7's docs to hand the tool server to the CLI
  (Claude Code `--mcp-config`, Codex `-c mcp_servers.plenipo.*`), or keep the tool
  conversation-only and say so in `tool_posture`.

## 6. The output parser and the normalized result

```rust
fn parser(&self, request: &TurnRequest) -> Box<dyn TurnParser>;

trait TurnParser {
    fn line(&mut self, text: &str, truncated: bool) -> Parsed;   // one stdout line
    fn stderr(&mut self, text: &str);                           // kept to explain failures
    fn finish(&mut self, end: &ProcessEnd) -> TurnResult;       // exactly one result per turn
}
```

Build the parser on the shared `TurnState`, as both adapters do. For each stdout line:

1. Not JSON: `return self.state.malformed_line(truncated)`. The line is ignored and counted, and
   one warning is shown per turn. It must never stop the turn or panic.
2. An event type you do not know: `self.state.unknown += 1` and ignore it. CLIs add events.
3. Otherwise map it to normalized `AgentEvent`s and `self.state.understood += 1`:

| Normalized event                   | Claude Code                             | Codex                                          |
| ---------------------------------- | --------------------------------------- | ---------------------------------------------- |
| `SessionStarted` (confirms resume) | `system`/`init` (`session_id`, `model`) | `thread.started` (`thread_id`)                 |
| `TextDelta` (live only)            | `stream_event` text deltas              | none (`streaming_text: false`)                 |
| `Message`                          | `assistant` text blocks                 | `item.completed` / `agent_message`             |
| `Reasoning`                        | none                                    | `item.completed` / `reasoning`                 |
| `ToolUse` / `ToolResult`           | `tool_use` / `tool_result` blocks       | `command_execution`, `mcp_tool_call`, …        |
| `Usage`                            | `result.usage`                          | `turn.completed.usage`                         |
| completion: `state.completed`      | `result` with `subtype: success`        | `turn.completed`                               |
| error: `state.error`               | `result` with `is_error`                | `turn.failed` (and `error` events as warnings) |

Keep the provider session ID in `state.provider_session_id` and the final answer in
`state.final_text` (or the last `Message`). If the reported session differs from the one
requested, add a warning `Notice`, as both adapters do.

`finish` normally just calls `self.state.finish(end)`, which applies the shared rules
(ADR-007 §7):

- Cancelled, timed out, interrupted, and "could not start" map to their own outcomes.
- A reported completion is `Completed`.
- A reported error is classified by `classify_error`:
  - usage-limit wording becomes `UsageLimited`;
  - sign-in wording becomes `AuthRequired`;
  - network wording becomes `ProviderUnavailable`;
  - anything else is `Failed`.
- No result at all is `MalformedOutput` or `Crashed`, explained by the stderr tail.

If the new CLI's usage-limit or sign-in messages (recorded in step 0) are not matched, add their
phrases to `classify_error`. That list is shared, so keep the phrases specific.

**Billing during the turn.** A tool with `billing_checked_per_turn` stops the turn as soon as its
stream reports a credential that is not the subscription. It returns
`Parsed { stop: Some(Stop { outcome: BillingNotAllowed, reason }) }` and sets `state.stop`. When
billing was not confirmed up front, it also stops if output arrives before the credential report.
See Claude Code's `init` and `unconfirmed_billing`.

The contract suite feeds each parser junk: HTML, broken JSON, a JSON array, an unknown event,
and an over-long line. It checks that each line is ignored and counted, never stops the turn,
and that `finish` gives exactly one result for every way a process can end. The fake-CLI checks
also make each tool report a usage limit and an expired sign-in, and expect `UsageLimited` and
`AuthRequired`.

## 7. What it can do: the model capability discovery contract

```rust
fn capabilities(&self) -> RuntimeCapabilities {
    RuntimeCapabilities {
        streaming_text: false,
        resume: true,
        cancel: true,
        structured_results: true,
        billing_checked_per_turn: false,
        tool_posture: "Read-only sandbox: Codex may run read-only commands but cannot write …".into(),
        effort_levels: ULTRA.to_vec(),                          // lowest first
        known_models: vec![
            KnownModel::new("gpt-6-sol", "GPT-6-Sol", ULTRA),   // name, label, its own levels
            KnownModel::new("gpt-6-luna", "GPT-6-Luna", MAX),
            // …
        ],
    }
}
fn checked_version(&self) -> &'static str { "0.157.1" }
```

- `tool_posture` is shown on screen. Write it in plain words: what the worker can and cannot do.
- `effort_levels` are the levels the CLI's effort setting accepts, lowest first, or empty when it
  has none. Plenipo passes a level only if the tool lists it.
- `known_models` are the models the CLI itself offers (its picker, `--help`, or its own aliases
  such as Claude Code's `fable`, `opus`, `sonnet`, `haiku`), most capable first. Each one has:
  - the exact name the model option takes;
  - the label the CLI shows;
  - its own effort levels, a subset of the tool's. Haiku has none; GPT-6-Luna stops at max.

  Plenipo offers them in its model menus and never adds them to the owner's list. Routing uses
  `effort_levels_for(model)`.

- `checked_version()` is the CLI version those lists were checked against. Update the lists and
  the version together whenever you re-check. A newer CLI may offer models not listed yet, and
  the owner can still type a name.
- Plenipo does not ask the CLI for its models at run time (ADR-011). The list is data in the
  adapter, checked by hand.

The contract suite checks:

- the version looks like a CLI version;
- the effort levels are in order;
- every model name passes `validate_model` and is listed once;
- each model's levels are within the tool's.

## 8. The fake CLI persona

CI has no accounts, so every test drives
[`plenipo-fake-agent`](../../crates/runtime/src/bin/plenipo-fake-agent.rs). The binary answers as
the CLI its file is named after. Add the new tool's persona:

1. Write `fn <name>(args: &[String]) -> i32`, modelled on `claude` and `codex`, and add it to
   `PERSONAS` under the tool's `executable_name`.
2. `--version`: print a fake version in the real format.
3. The status command: answer as the real CLI does for each `auth_mode()`:
   - `subscription` (default);
   - `api-key`;
   - `signed-out`;
   - `unknown-status` (the command fails).

   Use the wording recorded in step 0.

4. A turn:
   1. `record_invocation(args)`, then check the flags the adapter must pass.
   2. `read_prompt()` from stdin.
   3. `view(&prompt)` for Liaison messages.
   4. Remember the session with `remember()`, or load it for a resume (fail as the real CLI does
      when it is missing).
   5. Print the real stream format and answer with `answer(…)`, so handoff markers work.
5. Markers, with the real CLI's own error texts: `[crash]`, `[malformed]`, `[usage-limit]`,
   `[auth-expired]`, `[offline]`, `[slow]`, `[unknown]`, `[big]`, `[delay:MS]`.

The test helpers install every persona the fake lists (`plenipo-fake-agent --personas`):

- the Rust harnesses through `personas()`;
- the end-to-end tests through `installFakeTools(home)` in `tests/e2e/lib/app.mjs`.

Nothing else needs editing to put the new tool on the tests' PATH.

## 9. Register it

In [`crates/runtime/src/agent/mod.rs`](../../crates/runtime/src/agent/mod.rs), add `pub mod <tool>;`
and one line in `builtin_adapters()`, in display order:

```rust
pub fn builtin_adapters() -> Vec<std::sync::Arc<dyn RuntimeAdapter>> {
    vec![
        std::sync::Arc::new(claude_code::ClaudeCode),
        std::sync::Arc::new(codex::Codex),
        std::sync::Arc::new(gemini::Gemini),   // for example
    ]
}
```

That is all the registration there is. The following pick the tool up on their own:

- the AI tools page and the model menus;
- routing and the handoff directory;
- the desktop's IPC tests;
- the contract suite.

## 10. Setup guide and screen text

- **Setup guide.** Add a row to the table in
  [`setup.md` §3](setup.md#3-ai-tools-claude-code-and-codex-phase-3-optional) with the official
  install command and the subscription sign-in command. Add notes for anything the owner needs:
  the Windows build Plenipo runs, and what workers can do. Use plain words.
- **Screen text that names the AI tools.** Today these still say "Claude Code and Codex". The
  first tool branch updates them to cover every AI tool:
  - the AI tools page heading and intro (`apps/desktop/src/components/AgentRuntimeCards.tsx`);
  - the Workers intro (`apps/desktop/src/views/WorkersView.tsx`);
  - Settings: the "Permissions" line and "Programs Plenipo runs"
    (`apps/desktop/src/views/SettingsView.tsx`; Phase 7 rewrites the permissions text);
  - the Activity trail's tool names (`TOOL_NAMES` in `apps/desktop/src/ledger/format.ts`);
  - the README's introduction and quick start.

## Checklist for a tool branch

Copy this into `docs/phases/ai-tools-<tool>-checklist.md` and tick it as you go.

```markdown
## Bar (ADR-014 — adding AI tools ahead of Phase 15), checked on the real CLI

- [ ] Official CLI with a non-interactive mode (prompt on stdin, exits by itself) — evidence:
- [ ] Structured or streaming output (session ID, answer, errors) — evidence:
- [ ] Subscription sign-in only; no API key or password ever needed — evidence:
- [ ] Sign-in status check tells a subscription from an API key — evidence:
- [ ] Stable execution (version flag, resume by ID, same result on repeat) — evidence:
- [ ] Least privilege: flags that stop writes and network — evidence:
- [ ] CLI version checked: …

## Adapter (`crates/runtime/src/agent/<tool>.rs`)

- [ ] Identity: id, label, AI company, install and sign-in hints (plain words)
- [ ] Executable: name, known locations, Windows `.exe` handling, version
- [ ] Sign-in check: status command and classification (subscription, API key, signed out,
      unknown); no account names or key fragments kept
- [ ] Environment: config directory and `NETWORK_ENV` only; no API-key or billing variables
- [ ] Turn arguments: non-interactive, JSON output, least privilege, model, effort, new and
      resumed session
- [ ] Parser: every event type from the recorded output; malformed and unknown lines ignored;
      usage-limit and sign-in errors classified
- [ ] Capabilities: tool posture, effort levels, known models with their own effort levels,
      `checked_version()`
- [ ] Unit tests in the module, using the outputs recorded above
- [ ] After Phase 7 is on main: Plenipo's tool server wired in, or conversation-only stated

## Around it

- [ ] Persona in `plenipo-fake-agent` (version, status for each sign-in mode, turn, resume,
      markers)
- [ ] Registered in `builtin_adapters()`
- [ ] Setup guide §3 row and notes
- [ ] Screen text that names the AI tools updated (first tool branch)
- [ ] Contract suite passes (`cargo test -p plenipo-runtime --test contract`)
- [ ] Integration tests updated where they expect exactly two AI tools
- [ ] All pre-push checks from CLAUDE.md, and `pnpm e2e` against the release build
- [ ] Owner's check on Windows with the real CLI: one task, resume, cancel, and a sign-in that
      is refused (API key or signed out)
- [ ] Acceptance report `docs/phases/ai-tools-<tool>-acceptance-report.md`
```

## Writing a finding

When a tool fails the bar, the branch merges `docs/phases/ai-tools-<tool>-finding.md` instead of
an adapter. It has:

- **Tool and version checked**, and the date;
- **Bar item that failed**, quoting ADR-014;
- **Evidence**: the commands run and their output, or the documentation that shows it;
- **What would change the answer**: for example, "a status command that says whether the
  sign-in is a subscription", or "a non-interactive mode that reads the prompt from stdin";
- **Not done**: the workarounds considered and why ADR-014 rules them out (API key, unofficial
  client, driving the interactive screen).
