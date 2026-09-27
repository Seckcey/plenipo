# AI tools: Kimi — Implementation Checklist

**Status:** built and tested with the fake Kimi on `claude/ai-tools-kimi`; waiting for the
owner's check on Windows with the real CLI (see the
[acceptance report](ai-tools-kimi-acceptance-report.md), §6).

Adds Kimi Code, Moonshot AI's official coding CLI, as an AI tool under ADR-014 (the rules for
adding AI tools ahead of Phase 15), ADR-015 (running AI tools over ACP), and ADR-018 (Kimi over
ACP, with its file reads and writes going through Plenipo), accepted by the owner on 2026-09-26.
ADR-018 was numbered ADR-016 when accepted; it was renumbered on 2026-09-27 because `main`
already uses ADR-016 for the Development department. Guide:
[`adding-an-ai-tool.md`](../development/adding-an-ai-tool.md), §11. The owner checked the real CLI
on Windows 11 with a Kimi subscription; the raw outputs are in
[`crates/runtime/tests/fixtures/kimi-0.34.0/`](../../crates/runtime/tests/fixtures/kimi-0.34.0/README.md).

## Step 0: the bar, checked on the real CLI

| Check                  | Kimi Code 0.34.0                                                                                                                                                                                                                                                                                                                                                                                                                                                      | Bar item | Result      |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ----------- |
| Install                | A native `kimi.exe` in `%USERPROFILE%\.kimi-code\bin` (the official installer).                                                                                                                                                                                                                                                                                                                                                                                       | 1        | Pass        |
| Version                | `kimi --version` → `0.34.0`; ACP `initialize` also reports `agentInfo.version`.                                                                                                                                                                                                                                                                                                                                                                                       | 5        | Pass        |
| One task, no questions | Prompt mode (`kimi -p "…" --output-format stream-json`) runs one task and exits, but takes the prompt only as an argument (`-p -` sends the text `-`). **ACP (`kimi acp`) takes the prompt as a JSON-RPC message on stdin**, one process per turn.                                                                                                                                                                                                                    | 1, 2     | Pass (ACP)  |
| Output                 | Prompt mode: one JSON object per line (version, answer, tool calls, session ID). ACP: JSON-RPC over stdio with streamed `session/update` messages (thoughts, answer chunks, tool calls, usage) and `stopReason` at the end.                                                                                                                                                                                                                                           | 2        | Pass        |
| Resume                 | Prompt mode: `-S <id>`. ACP: `session/load` in a new process replays the conversation; the next prompt remembered it.                                                                                                                                                                                                                                                                                                                                                 | 5        | Pass        |
| Sign-in status         | No status command. `kimi provider list` names each provider and its credential source (`managed:kimi-code type=kimi … source=oauth` = the Kimi subscription). ACP `initialize` lists `authMethods` (`login`, device code). Signed-out output: to capture.                                                                                                                                                                                                             | 3, 4     | Partial     |
| Errors                 | Unknown model: `Model "…" is not configured in config.toml.` Usage limit and expired sign-in: to capture (docs or source).                                                                                                                                                                                                                                                                                                                                            | 2        | To do       |
| Models and effort      | ACP `session/new` lists them: `kimi-code/kimi-for-coding` (K2.8 Preview), `kimi-code/kimi-for-coding-highspeed` (K2.7 Code Highspeed), `kimi-code/k3` (K3, default), `kimi-code/k3-256k` (K3-256k). Thinking: `low`, `high`, `max` for K3; `on`, `low` for K2.7 Code Highspeed. `session/set_config_option` switched the model and the next answer came from it.                                                                                                      | —        | Pass        |
| Least privilege        | **Prompt mode wrote a file without asking**, and `--plan` cannot be combined with `-p`. In ACP (mode `default`, "manual approvals"), with the client offering file access: every read, inside or outside the folder, came to the client as `fs/read_text_file` (refused, and not worked around); a write and a command (`echo`) each waited for `session/request_permission` (refused; nothing written or run). Modes: `default`, `plan` (read-only), `auto`, `yolo`. | —        | Pass (ACP)  |
| Credentials in the env | No `KIMI*` or `MOONSHOT*` variables set; the subscription token lives in `%USERPROFILE%\.kimi-code` (never read by Plenipo).                                                                                                                                                                                                                                                                                                                                          | 3        | Pass so far |

## Bar (ADR-014 — adding AI tools ahead of Phase 15), checked on the real CLI

- [x] Official CLI with a non-interactive mode (prompt on stdin, directly or over ACP per ADR-015;
      exits by itself) — Kimi Code (`kimi`) from Moonshot AI. `kimi -p` cannot read stdin
      (tested), so Plenipo uses `kimi acp`: the prompt goes in on stdin, and the process ends
      when stdin closes (exit code 0 in every capture).
- [x] Structured or streaming output (session ID, answer, errors) — ACP messages: the
      `session/new` answer carries the conversation ID and settings, `session/update` the text,
      thoughts, and tool calls, the `session/prompt` answer the stop reason. Kimi reports no
      token counts.
- [ ] Subscription sign-in only; no API key or password ever needed — `kimi login` (device code)
      signs in to the Kimi subscription (`source=oauth`); the owner is signed in this way.
      **Owner check:** tasks run on it.
- [x] Sign-in status check tells a subscription from an API key — `kimi provider list`: only
      `managed:kimi-code … source=oauth` counts; another source (an API key) is refused, and
      no Kimi subscription provider reads as signed out. The signed-out wording is still to
      capture (the adapter does not depend on it).
- [x] Stable execution (version flag, resume by ID, same result on repeat) — `kimi --version`
      (`0.34.0`), `session/load` with the conversation ID (checked on the real CLI).
      **Owner check:** resume and cancel in Plenipo.
- [x] Least privilege: flags that stop writes and network — no flag can switch Kimi's own tools
      off, so ADR-018: every file Kimi reads or writes comes to Plenipo and goes through Guard,
      its own shell is refused, and its mode is `default` (or `plan` for a worker without
      permissions). **Owner check:** Kimi's approved writes come to Plenipo, and its web tools
      (if any) ask first.
- [x] CLI version checked: 0.34.0.

## Adapter (`crates/runtime/src/agent/kimi.rs`)

- [x] Identity: `kimi`, "Kimi", Moonshot AI (`moonshot`), install and sign-in hints in plain words
- [x] Executable: `kimi`; `%USERPROFILE%\.kimi-code\bin\kimi.exe` (a real `.exe`),
      `~/.kimi-code/bin`, `~/.local/bin`; version from `kimi --version`
- [x] Sign-in check: `kimi provider list`; subscription (`managed:kimi-code`, `source=oauth`),
      API key, unrecognized, signed out; nothing but a short method name kept ("Kimi sign-in")
- [x] Environment: `NETWORK_ENV` only; no Kimi or Moonshot variable; Plenipo never reads
      `%USERPROFILE%\.kimi-code`
- [x] Turn arguments: `acp`; the conversation, mode, model, and thinking level travel in ACP
      messages
- [x] Parser: the shared ACP driver (`acp.rs`) with ADR-018's options — file access through
      Guard, session settings, allowed modes, `session/load` to resume, tool names in titles,
      and the refusal of models outside `kimi-code/…`
- [x] Capabilities: tool posture, thinking levels (low, high, max), the four `kimi-code/…`
      models (K3 first, Kimi's default), `checked_version()` = `0.34.0`
- [x] Unit tests in the module, using the recorded outputs (`tests/fixtures/kimi-0.34.0/`)
- [x] Plenipo's tool server wired in (ACP `mcpServers`), tested through the real broker and relay

## ADR-018 (Kimi over ACP, with its file reads and writes going through Plenipo) — what it asked for

- [x] §1 Kimi runs over ACP through the shared driver, one `kimi acp` per task
- [x] §2 `initialize` offers `fs.readTextFile` and `fs.writeTextFile`, and no terminal; each
      `fs/read_text_file` / `fs/write_text_file` is carried out by the broker's `read_file` /
      `write_file` path (folder, blocked files, redaction, record, Ask-me approvals); a worker
      with no grant has every file request refused; Grok keeps file access off
- [x] §3 Permission requests: Kimi's file changes allowed only when the grant offers
      `write_file` (the change then comes to Plenipo; one reported done without it stops the
      task); its shell always refused, with a note pointing to `run_command`; Plenipo's tool
      server allowed; anything else refused; never `allow_always`
- [x] §4 Mode `default`, or `plan` for a worker without permissions; `auto`, `yolo`, or any
      other mode stops the task
- [x] §5 Model and thinking level set with `session/set_config_option` after the conversation
      opens and before the prompt, each answer checked; the model list declared in the adapter
      and checked against Kimi's own list in a test
- [x] §6 `kimi provider list` before every task; only `kimi-code/…` models (the default named
      explicitly); no key variables; an authentication error reads as "sign-in required"

## Changes to shared code

- [x] Runtime contract: `ToolProvider::file_access`, `Parsed::files` (`FileRequest`),
      `TurnParser::file_answered`, and `ToolServer::tools` (the names the grant offers); the
      service carries file requests out while the task goes on
- [x] Broker: `Broker::file_request` (the same Guard path as a tool call, recorded with
      `fileRequest: true`); a read answers the file as it is, never cut short
- [x] Model names may carry one provider prefix (`kimi-code/k3`) in the runtime and the Ledger;
      still never a flag or a path (`../x`, `/x`, `C:/x`, `a/b/c` refused)
- [x] Contract suite: an ACP tool's model and effort may travel in its messages

## Around it

- [x] Persona in `plenipo-fake-agent` (`kimi`: version, `provider list` for each sign-in mode,
      ACP handshake, new/load, settings, prompt, file requests, permission requests, cancel,
      markers)
- [x] Registered in `builtin_adapters()`
- [x] Setup guide §3 row and notes; configuration doc; README; architecture overview; adapter
      guide §11
- [x] Screen text that names the AI tools updated (AI tools page, Workers, Settings, Activity)
- [x] Contract suite passes (`cargo test -p plenipo-runtime --test contract`)
- [x] Integration tests: runtime (every shared test, plus Kimi's settings, refusals, modes, and
      models), Guard broker (reads through Guard, redaction, writes by a developer, refusals, a
      change around Plenipo)
- [x] End-to-end: Kimi on the AI tools page, a Kimi task, its models in the model menu
- [x] All pre-push checks from CLAUDE.md
- [x] `pnpm e2e` against the release build (45 of 45 passed, Linux)
- [ ] Owner's check on Windows with the real CLI: one task, resume, cancel, files through
      Guard, and a refusal —
      [acceptance report §6](ai-tools-kimi-acceptance-report.md#6-owner-check-on-windows-about-20-minutes)
- [x] Acceptance report `docs/phases/ai-tools-kimi-acceptance-report.md`

## Still to check

- The texts of a usage limit and an expired sign-in (from Kimi's docs or source, or seen in use).
  Until then, Plenipo's general wording rules classify them.
- What `kimi provider list` prints when signed out (Plenipo reads "no Kimi subscription
  provider" as signed out, whatever the wording).
- How Kimi asks to use a tool of Plenipo's tool server (Plenipo accepts the tool's own name, or
  the name with `plenipo__` / `mcp__plenipo__` before it).
- Whether an approved Kimi write always comes to Plenipo as `fs/write_text_file` (if it does
  not, Plenipo stops the task and says so).
- Whether Kimi has web tools that run without asking in its `default` mode.
- `kimi provider list --json` is not used: it prints Kimi's raw settings, which could hold a
  key.
