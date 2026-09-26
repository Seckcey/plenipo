# AI tools: Grok — Implementation Checklist

**Status:** built and tested with the fake Grok; waiting for the owner's check on Windows with
the real CLI (see the [acceptance report](ai-tools-grok-acceptance-report.md), §6).

Branch `claude/ai-tools-grok`. Rules: ADR-014 (adding AI tools ahead of Phase 15) and ADR-015
(running AI tools over ACP), accepted by the owner on 2026-09-26. Guide:
[`adding-an-ai-tool.md`](../development/adding-an-ai-tool.md), including §11 (a tool that talks
over ACP). Evidence from the real CLI: [`evidence/ai-tools-grok/`](evidence/ai-tools-grok/README.md).

## Bar (ADR-014 — adding AI tools ahead of Phase 15), checked on the real CLI

- [x] Official CLI with a non-interactive mode (prompt on stdin, directly or over ACP per ADR-015;
      exits by itself) — Grok Build (`grok`) from xAI. `grok -p` cannot read stdin (tested; xAI's
      guide says so), so Plenipo uses `grok agent --no-leader stdio` (ACP): the prompt goes in on
      stdin, and the process ends when stdin closes. Signed-out ACP exchange recorded.
- [x] Structured or streaming output (session ID, answer, errors) — ACP messages: the
      `session/new` answer carries the conversation ID, `session/update` the text and tool use,
      the `session/prompt` answer the stop reason and usage; errors are JSON-RPC errors
      (`-32000 Authentication required` recorded signed out).
- [ ] Subscription sign-in only; no API key or password ever needed — `grok login` with an X
      account (SuperGrok or X Premium Plus, per xAI). `GROK_DISABLE_API_KEY_AUTH=1` makes Grok
      refuse `XAI_API_KEY` and a key set on a model in its settings (tested with fake keys).
      **Owner check:** a real subscription sign-in.
- [ ] Sign-in status check tells a subscription from an API key — `grok models`, first line:
      "You are not authenticated." / "You are using XAI_API_KEY." / "Model '…' is using its own
      API key." / "You are authenticated via deployment key." (recorded or in the program).
      **Owner check:** the signed-in wording (Plenipo accepts a line saying "logged in" or
      "signed in"; anything else is refused and quoted on the AI tools card).
- [x] Stable execution (version flag, resume by ID, same result on repeat) — `grok --version`
      (`grok 1.0.41 (4220f3b224a6)`), `session/resume` with the conversation ID (advertised in
      `initialize`), exit codes documented (0, 1, 130, 143). **Owner check:** resume signed in.
- [x] Least privilege: flags that stop writes and network — no flag in ACP mode; instead an agent
      profile with none of Grok's own tools (`_meta.agentProfile`), `GROK_SUBAGENTS=0`,
      `GROK_MEMORY=0`, `GROK_WEB_FETCH=0`, the Claude Code and Cursor switches (checked with
      `grok inspect --json` and a task's tool list), and Plenipo refusing every tool request but
      its own tool server's. Grok's kernel sandbox is Linux and macOS only. **Owner check:** Grok
      does not use its own tools.
- [x] CLI version checked: 1.0.41 (stable), stdin checks repeated on 1.0.42 (alpha).

## Adapter (`crates/runtime/src/agent/grok.rs`)

- [x] Identity: `grok`, "Grok", xAI (`xai`), install and sign-in hints in plain words
- [x] Executable: `grok`; `%USERPROFILE%\.grok\bin\grok.exe` (a real `.exe`), `~/.grok/bin`,
      `~/.local/bin`; version from `grok --version`
- [x] Sign-in check: `grok models`, first line; subscription, API key, signed out, unrecognized;
      no account names kept (method: "Grok sign-in (X account)")
- [x] Environment: `GROK_HOME` and `NETWORK_ENV` passed; `GROK_DISABLE_API_KEY_AUTH=1` and the
      least-privilege switches set; no API-key or billing variables
- [x] Turn arguments: `agent --no-leader [-m MODEL] [--reasoning-effort LEVEL] stdio`; the
      conversation ID travels in ACP messages
- [x] Parser: the shared ACP driver (`acp.rs`) — every message type seen; unknown messages
      ignored and counted; usage-limit and sign-in errors classified; never sends `authenticate`
- [x] Capabilities: tool posture, effort levels, `grok-4.6` (low–extra high) and `grok-4.5`
      (low–high), `checked_version()` = `1.0.41`
- [x] Unit tests in the module, using the recorded outputs (`include_str!` from the evidence)
- [x] After Phase 7 is on main: Plenipo's tool server wired in (ACP `mcpServers`), tested
      through the real broker and relay

## Around it

- [x] Persona in `plenipo-fake-agent` (`grok`: version, status for each sign-in mode, ACP
      handshake, new/resume/load, prompt, permission requests, cancel, markers)
- [x] Registered in `builtin_adapters()`
- [x] Setup guide §3 row and notes; configuration doc (variables)
- [x] Screen text that names the AI tools updated (AI tools page, Workers, Settings, Activity)
- [x] Contract suite passes (`cargo test -p plenipo-runtime --test contract`)
- [x] Integration tests updated where they expect exactly two AI tools
- [x] All pre-push checks from CLAUDE.md, and `pnpm e2e` against the release build
- [ ] Owner's check on Windows with the real CLI: one task, resume, cancel, and a sign-in that
      is refused (API key or signed out) — [acceptance report §6](ai-tools-grok-acceptance-report.md#6-owner-check-on-windows-about-15-minutes)
- [x] Acceptance report `docs/phases/ai-tools-grok-acceptance-report.md`

## ADR-015 (running AI tools over ACP) — what it asked for

- [x] Supervisor keeps stdin open for a task that talks (`StdinFeed`); one-way tasks unchanged
- [x] `TurnParser::open` / `Parsed::send` / `Parsed::close_input` / `TurnParser::cancel`
- [x] One shared ACP driver (`crates/runtime/src/agent/acp.rs`), protocol version 1, no new
      dependency
- [x] Permission requests answered by Plenipo: its tool server allowed, everything else refused
      and noted in the task's activity
- [x] Cancel sends `session/cancel` and waits up to five seconds before ending the process tree
- [x] Fake ACP persona and tests (driver unit tests, contract suite, integration tests, Guard
      broker test, end-to-end)
- [ ] §6 condition before merging: the owner's check shows that `GROK_DISABLE_API_KEY_AUTH=1`
      blocks a key set on a model when signed in (shown signed out with a fake key)
