# AI tools: Grok — Acceptance Report

|              |                                                                                                                                                                                         |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Track**    | AI tools: Grok (xAI), under ADR-014 (adding AI tools ahead of Phase 15) and ADR-015 (running AI tools over ACP)                                                                         |
| **Branch**   | `claude/ai-tools-grok` (draft pull request #15)                                                                                                                                         |
| **Verified** | Locally on Linux: `pnpm check`, `cargo fmt/clippy/test`, `pnpm bindings`, full `pnpm e2e` against the release build. The real Grok CLI 1.0.41, signed out. GitHub CI: the pull request. |
| **Date**     | 2026-09-26                                                                                                                                                                              |
| **Result**   | **Ready for the owner's check on Windows (§6).** Built and tested end to end with the fake Grok. Merging waits for the owner's check with the real, signed-in CLI.                      |

Test totals: **533 Rust** (Linux; 514 on `main` before this branch, plus 11 for the ACP driver,
6 for the Grok adapter, 1 supervisor test, and 1 Guard broker test; the contract suite and the
runtime integration tests now also run for Grok) · **138 frontend** · **40 end-to-end** against
the release build (one new Grok task through Workers; the AI tools and Settings → AI models
checks now include Grok).

## 1. How Grok got here

1. **Step 0 on the real CLI** (2026-09-26). xAI's official CLI is Grok Build (`grok`,
   1.0.41 stable, 1.0.42 alpha), installed with `https://x.ai/cli/install.sh`
   (Windows: `install.ps1`, a real `grok.exe`). It was run without signing in; the recorded
   output is in [`evidence/ai-tools-grok/`](evidence/ai-tools-grok/README.md).
2. **It failed ADR-014's bar item 1.** Its one-task mode, `grok -p`, does not read the prompt
   from stdin:
   - `-p` needs a value, and `-p -` is the literal prompt "-" (with empty input it still
     reaches the sign-in check, while `-p ""` is refused as empty);
   - `--prompt-file -` and `--prompt-json -` read "-" literally, and `--prompt-file /dev/stdin`
     works only on Linux and macOS;
   - xAI's guide says: "Headless mode does not read piped stdin into the prompt."

   A finding was written (ADR-014 §4). The owner kept the stdin rule and asked for the ACP route
   instead.

3. **ADR-015 (running AI tools over ACP)**, accepted by the owner: `grok agent stdio` takes
   everything, the prompt included, on stdin over ACP (JSON-RPC 2.0, one message per line), so
   the prompt never goes on the command line. The finding was replaced by this adapter.

## 2. What was built

| Deliverable                  | Where                                                                                                                                                                                                                                                                                              |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Decision record              | [ADR-015 (running AI tools over ACP)](../adr/ADR-015-acp-ai-tools.md), accepted. ADR-014's bar item 1 and ADR-007 point to it from their status lines.                                                                                                                                             |
| Tasks that talk (ADR-015 §3) | `StdinFeed` in `crates/runtime/src/profile.rs` and the supervisor keep stdin open. `TurnParser::open`, `Parsed::send`, `Parsed::close_input`, `TurnParser::cancel` in `adapter.rs`; the service writes what the parser sends, and cancel asks first (five seconds) before ending the process tree. |
| Shared ACP driver            | [`crates/runtime/src/agent/acp.rs`](../../crates/runtime/src/agent/acp.rs): `initialize` → `session/new` (or `session/resume` / `session/load`) → `session/prompt`; permission answers; never `authenticate`; unknown messages ignored and counted.                                                |
| Grok adapter                 | [`crates/runtime/src/agent/grok.rs`](../../crates/runtime/src/agent/grok.rs), registered in `builtin_adapters()`.                                                                                                                                                                                  |
| Fake Grok                    | The `grok` persona of `plenipo-fake-agent` speaks ACP, asks permission before each tool call, and stops on `session/cancel`.                                                                                                                                                                       |
| Screen text                  | AI tools page ("Your AI tools"), Workers intro, Settings (programs Plenipo runs), and the Activity trail's tool names now include Grok. Nothing about ACP appears on screen.                                                                                                                       |
| Docs                         | Setup guide §3 (install, sign-in, what Plenipo checks), configuration (variables), adapter guide §11 (a tool that talks over ACP), architecture overview §6, README.                                                                                                                               |

### How Plenipo runs Grok

- **Launch:** `grok agent --no-leader [-m MODEL] [--reasoning-effort LEVEL] stdio`, one process
  per task, in the conversation's own folder. `--no-leader`: never Grok's shared background
  process, which would run outside Plenipo's supervision.
- **Sign-in and billing** (ADR-007 §4):
  - before every task, `grok models`; its first line names the credential;
  - only a Grok sign-in lets a task run (an ACP task does not report its credential);
  - `GROK_DISABLE_API_KEY_AUTH=1` on every launch makes Grok refuse API keys, including a key
    set on a model in `config.toml` (tested with fake keys, signed out);
  - `XAI_API_KEY`, `GROK_CODE_XAI_API_KEY`, `GROK_DEPLOYMENT_KEY`, and Grok's auth-provider,
    OIDC, and endpoint variables are never passed.
- **Least privilege:** an agent profile with none of Grok's own tools, sent with the session
  (checked with a fake key and a local stand-in for xAI: the model is offered no tools, or only
  `search_tool` and `use_tool` for Plenipo's tool server; see
  [`acp-tools.txt`](evidence/ai-tools-grok/acp-tools.txt));
  `GROK_SUBAGENTS=0`, `GROK_MEMORY=0`, `GROK_WEB_FETCH=0`, and `GROK_BACKEND_SEARCH=0` (xAI's
  own web and X search, which run on xAI's side); the Claude Code and Cursor switches
  off (checked: a task then loads none of the owner's skills). A worker with permissions gets
  Plenipo's tool server in `session/new`; Grok reaches it through its `use_tool`, asks first,
  and Plenipo allows only that server (Guard decides inside each call).
- **Models:** `grok-4.7` (Grok's default), `grok-4.7-build-fast` ("Grok 4.7 Fast"), and
  `grok-4.6` (Low, Medium, High, Extra high), and `grok-4.5` (Low, Medium, High), as
  Grok 1.0.41 lists them.

## 3. Tests

| Test                                                                  | Covers                                                                                                                                                                                                                     |
| --------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `acp.rs` unit tests (11)                                              | The exchange, using Grok's real `initialize` answer and real signed-out error; resume vs load; permission answers; refused client methods; cancel; errors; usage; unknown messages.                                        |
| `grok.rs` unit tests (6)                                              | Sign-in check from real `grok models` output; version; arguments; the profile; variables; models and effort.                                                                                                               |
| Contract suite (`contract.rs`, 7)                                     | Grok passes all seven, including a full task through the fake Grok with the prompt on stdin, usage-limit and sign-in errors.                                                                                               |
| Runtime integration (`agents.rs`)                                     | Grok joins every loop: a task with streamed text and a result, resume, cancel (Grok stops itself; no hard kill), failures, usage limits, large and unknown output, restart, shutdown, and a task that waits and continues. |
| Supervisor (`a_stdin_feed_keeps_stdin_open_until_its_sender_is_gone`) | Stdin stays open while a task talks, in order, and closes when it is done.                                                                                                                                                 |
| Guard (`a_grok_worker_uses_plenipo_tools_over_acp_and_nothing_else`)  | Through the real broker and relay: a Grok Supervisor lists and uses Plenipo's tools, a write its role does not allow is blocked, and Grok's request for its own tool is refused and noted.                                 |
| Desktop IPC                                                           | The AI tools list and the default models follow the registered tools (two tests that still said two tools now follow `builtin_adapters()`).                                                                                |
| End to end (`agents.e2e.mjs`, `routing.e2e.mjs`)                      | Grok appears under AI tools (ready, version, Grok sign-in); a Grok task runs through Workers; Settings → AI models → Add a model lists Grok's four models.                                                                 |

## 4. Changes to shared code, and why

Settings, the Router, and the Organization view needed no Grok-specific code. Shared code
changed only where ADR-015 said it would:

- **The task contract** (§2) — the only way to send the prompt after the conversation opens.
- **Two contract tests.** The resume test now also looks in a task's messages for the
  conversation ID, because an ACP tool gets it there, not in its arguments. The variables test
  allows a fixed off-switch Plenipo sets to `1` (`GROK_DISABLE_API_KEY_AUTH`); every other name
  is still checked for credential words.
- **`TurnRequest::working_dir`** — ACP names the conversation's folder in its messages.
- **Tests that assumed two AI tools** (desktop IPC, Workforce, Guard broker, runtime) now follow
  the registered tools, or name Grok where the check is per tool.

## 5. Security notes

- The prompt never appears in a program's arguments (tested by the contract suite and the
  integration tests).
- Plenipo never sends ACP's `authenticate`, which could start a browser sign-in; the fake Grok
  records it if it happens, and the tests check it never does.
- Plenipo offers Grok no file or terminal access of its own (`fs` and `terminal` off in
  `initialize`) and refuses any client method it does not offer.
- Grok's tool requests are answered by Plenipo, never the owner: its tool server yes, anything
  else no, and each refusal is recorded in the task's activity.
- Not verifiable until signed in: whether the agent profile removes every one of Grok's own tools
  on the real CLI, and the exact shape of Grok's permission requests. The owner check covers
  both (§6, steps 9–10).

## 6. Owner check on Windows (about 15 minutes)

Use this branch's build (`pnpm build`, then `target\release\plenipo-desktop.exe`). Before
sending anything back, remove your email, X handle, and any account or team IDs. If a step
prints a key, do not paste it.

1. Install Grok: `irm https://x.ai/cli/install.ps1 | iex`. Open a new PowerShell window, then run
   `grok --version` (1.0.41 or newer).
2. Sign in: `grok login`, with the X account that has SuperGrok or X Premium Plus.
3. Check for keys. Each of these should print nothing:
   `echo $env:XAI_API_KEY`, `echo $env:GROK_CODE_XAI_API_KEY`, and
   `Select-String -Path "$env:USERPROFILE\.grok\config.toml" -Pattern 'api_key|env_key|models_base_url|auth_provider'`.
4. Run `grok models` and copy its first line.
5. Start Plenipo. Open **AI tools** and choose **Re-check**. The Grok card should say **Ready**
   and "Signed in (subscription) · grok.com sign-in". If it says Plenipo could not
   confirm the sign-in, copy the line it quotes and stop here; that line is all I need.
6. In **Workers**, pick **Grok** and start: _"Say hello and tell me what you are."_ It should
   finish with an answer and token counts.
7. Follow up in the same conversation: _"What did I just ask you?"_ It should remember
   (resume).
8. Start another Grok task: _"Count slowly from 1 to 300, one number per line."_ Choose
   **Cancel** while it runs. It should read **Cancelled** within about five seconds. Then follow
   up with _"Continue."_ It should answer in the same conversation.
9. **Plenipo's tools in a project.** In **Organization**, select the **Website Supervisor**.
   In its details panel, under **Project: Website**:
   - **Allowed AI tools** must include Grok, and **Folder** must not say "None". If either is
     wrong, choose **Edit project**, tick **Grok** under **Allowed AI tools**, type a folder that
     has a README.md in it into **Project folder** (for example `D:\projects\website`), and save.
     A project made before Grok existed does not allow Grok yet.
   - Choose **Edit title, AI tool, or model**, pick **Grok**, and save.
   - In **Give an objective**, type _"Read README.md and summarize it. Then run `dir`
     yourself."_ and choose **Give objective**.

   In **Activity** you should see "Permissions given to Website Supervisor in …" (with **Read
   files**), then "Website Supervisor: read README.md", and the answer summarizes the README.
   Grok must not run `dir` with its own tools: it says it cannot, or **Activity** shows "Grok
   asked to use … Plenipo refused it". It must not mention X or web search. If **Activity** says
   "… has permissions but got no tools …", copy that line.

10. If any Grok task above did not finish as described, open **AI tools**, find its program
    entry (for example "Grok · task 1"), and copy its output.
11. Refused sign-ins. Close Plenipo first. Work in PowerShell in your Plenipo folder.
    - **A key in your environment never reaches Grok.** Run
      `$env:XAI_API_KEY = "xai-FAKE-not-a-real-key"`, then start Plenipo from that window
      (`.\target\release\plenipo-desktop.exe`). Open **AI tools** and choose **Re-check**: Grok
      stays **Ready** with "grok.com sign-in", because Plenipo never passes the key on. In
      **Workers**, give Grok _"Reply with ok."_: it finishes normally. Close Plenipo, then run
      `Remove-Item Env:XAI_API_KEY`.
    - **A key in Grok's own settings is refused while signed in** (ADR-015 §6). Run
      `notepad "$env:USERPROFILE\.grok\config.toml"` (let Notepad create the file if it asks) and
      add these two lines at the end, then save:
      `[model."grok-4.6"]` and `api_key = "xai-FAKE-not-a-real-key"`. Run `grok models` and
      copy its first line. Then run
      `$env:GROK_DISABLE_API_KEY_AUTH = "1"; grok -p "Reply with ok." -m grok-4.6; Remove-Item Env:GROK_DISABLE_API_KEY_AUTH`
      and copy everything it prints. It passes if Grok answers "ok" or says it is not signed in;
      it fails if the error mentions `401`, `Auth: ApiKey`, or "Incorrect API key" (the fake key
      was sent to xAI). Start Plenipo, open **AI tools**, choose **Re-check**, and copy the Grok
      card's text ("API key — not allowed" when `grok models` names the key). Close Plenipo,
      delete the two lines, save, start Plenipo, and **Re-check**: **Ready**.
    - **Signed out.** Run `grok logout`, then **Re-check**: the Grok card says **Not signed in**
      with the hint to run `grok login`. Run `grok login`, sign in again, and **Re-check**:
      **Ready**.
12. Send back:
    - the line from step 4, and the Grok card's text from step 5;
    - for any task that did not finish as described, its result and the output from step 10;
    - from step 9: what Grok answered, and any **Activity** line about permissions or refusals;
    - from step 11: the Grok card's text in each case, the first line of `grok models` with the
      key in `config.toml`, and what the `grok -p` command printed.

## 7. Not verified here

Everything below needs a signed-in Grok, which was never used here:

- a real task over ACP: whether the conversation opens without ACP's sign-in message, the text,
  tool calls, and usage fields a signed-in Grok sends, and its usage-limit wording;
- that Grok asks before using a tool server, and that `GROK_BACKEND_SEARCH=0` removes xAI's
  web and X search for a signed-in model (the agent profile removing Grok's own tools was
  checked here with a fake key: [`acp-tools.txt`](evidence/ai-tools-grok/acp-tools.txt));
- that `GROK_DISABLE_API_KEY_AUTH=1` blocks a key set on a model while signed in (ADR-015 §6;
  shown signed out).

If a real message differs from the fake Grok's, the fix belongs in `acp.rs` or `grok.rs` and
the persona, with the recorded output added to the evidence.

## 8. Owner results

- **Step 4 (2026-09-26):** `grok models` on the owner's machine starts with "You are logged in
  with grok.com." ([`models-signed-in.txt`](evidence/ai-tools-grok/models-signed-in.txt)).
  Plenipo reads that as a subscription sign-in ("grok.com sign-in" on the AI tools card).
  Signed in, Grok offers four models: `grok-4.7` (default), `grok-4.7-build-fast`, `grok-4.6`,
  and `grok-4.5`. Signed out it lists only the last two.
- **Models and effort (2026-09-26):** the owner's Grok was 1.0.13 and was updated to 1.0.41
  (`grok update`). Grok's ACP handshake, signed in, lists each model with its effort levels
  ([`models-effort-signed-in.txt`](evidence/ai-tools-grok/models-effort-signed-in.txt)):
  `grok-4.7`, `grok-4.7-build-fast` ("Grok 4.7 Fast"), and `grok-4.6` take Extra high, High,
  Medium, and Low; `grok-4.5` takes High, Medium, and Low. Plenipo's menu now lists all four;
  a test checks the adapter against this file. The setup guide now says to keep Grok up to
  date.
- **Steps 5–9 (2026-09-27):** steps 5–8 worked as described: the Grok card read Ready with
  the grok.com sign-in, and a task, its follow-up (resume), and cancel then continue all ran. Step 9 first
  stopped because the Website project, made before Grok existed, did not allow Grok (the step
  now says to tick it). Then the Supervisor's task finished, but Grok could not read README.md
  and said: "This session only has X post search". Two things follow:
  - **X search came through.** It is one of xAI's backend tools, added on xAI's side, so the
    agent profile does not reach it. Plenipo now sets `GROK_BACKEND_SEARCH=0`, Grok's documented
    switch for them. To be checked in the re-run of step 9.
  - **Plenipo's file tool did not reach the task.** Checked here, Grok starts a tool server
    given in `session/new` and offers it to the model
    ([`acp-tools.txt`](evidence/ai-tools-grok/acp-tools.txt)), so the likely cause is that the
    Supervisor got no tool server at all, which happens when the project has no folder (Claude
    Code would have had no way to read the file either). To be checked in the re-run: the
    project's **Folder**, and any "has permissions but got no tools" line in **Activity**.
