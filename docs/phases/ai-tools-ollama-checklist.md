# Ollama (cloud models) — AI tool checklist

**Status:** accepted on 2026-09-27 ([acceptance report](ai-tools-ollama-acceptance-report.md)).

Adds Ollama's **cloud models** as an AI tool under ADR-014 (the rules for adding AI tools ahead of
Phase 15). The owner has no dedicated graphics card, so Plenipo uses models that run on Ollama's
servers under the owner's ollama.com account (signed in with `ollama signin`; free plan), reached
through the Ollama service on the PC. No local models, no API key. The raw outputs are in
[`crates/runtime/tests/fixtures/ollama-0.34.4/`](../../crates/runtime/tests/fixtures/ollama-0.34.4/README.md).

## Step 0: the bar, checked on the real CLI

| Check                  | Ollama 0.34.4                                                                                                                                                                                                                            | Bar item | Result                       |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ---------------------------- |
| Install                | A native `ollama.exe` in `%LOCALAPPDATA%\Programs\Ollama` (the official installer); the Ollama service runs in the background.                                                                                                           | 1        | Pass                         |
| Version                | `ollama --version` → `0.34.4`; `GET /api/version` → `{"version":"0.34.4"}`.                                                                                                                                                              | 5        | Pass                         |
| One task, no questions | `ollama run <model>` reads the prompt from stdin and exits by itself. The service's `POST /api/chat` takes the prompt in the request body.                                                                                               | 1        | Pass                         |
| Output                 | The CLI prints plain text with terminal control codes. `POST /api/chat` streams one JSON object per line: thinking, answer, then `done` with `done_reason` and token counts.                                                             | 2        | Pass (service)               |
| Resume                 | Ollama keeps no conversations. `POST /api/chat` answered from the earlier messages Plenipo sent, so Plenipo keeps the history itself.                                                                                                    | 5        | Pass (Plenipo keeps history) |
| Sign-in status         | `POST /api/me` names the account and plan (`"plan":"free"`); signed out it answers `401` with a sign-in link, and a cloud request answers `401`.                                                                                         | 3, 4     | Pass                         |
| Errors                 | Signed out: `401` (`{"error":"unauthorized","signin_url":…}` from `/api/me`). The free plan's usage-limit message: recorded the first time it is seen.                                                                                   | 2        | Partial                      |
| Models and effort      | `gpt-oss:120b-cloud`: 117B, 131,072-token context, capabilities completion, tools, thinking; thinking levels low, medium (default), high. Other cloud models: to list.                                                                   | —        | Partial                      |
| Least privilege        | A model alone reads no files and runs nothing; it can only answer. Offered a `read_file` tool, it returned a `tool_calls` request (`{"path":"notes.txt"}`) and ran nothing itself, so Plenipo would decide every tool use through Guard. | —        | Pass                         |
| Credentials in the env | No `OLLAMA_*` variables set; no `OLLAMA_API_KEY`. The service listens on `127.0.0.1:11434` only. The sign-in key lives in `%USERPROFILE%\.ollama` (never read by Plenipo).                                                               | 3        | Pass                         |

## Decisions this needs

- **ADR-017 (running Ollama's cloud models through the Ollama service on this PC)**, accepted by
  the owner on 2026-09-27:
  Plenipo sends each task to the service on `127.0.0.1`, checks the sign-in with `/api/me`, runs
  only cloud models, keeps the conversation history itself, and starts Ollama workers with no
  tools (Plenipo's tools through Guard come as a follow-up).

## Owner's model choices

Checked with `ollama show` on the owner's PC (2026-09-27; `models/` in the fixtures) and listed
in the adapter:

| Model                       | Context   | Thinking levels             | Free plan                |
| --------------------------- | --------- | --------------------------- | ------------------------ |
| `gpt-oss:120b-cloud`        | 131,072   | low, medium (default), high | Answered (low, high, on) |
| `nemotron-3-ultra:cloud`    | 262,144   | on or off only (on)         | Answered (low, high, on) |
| `kimi-k3:cloud`             | 1,048,576 | low, high, max (default)    | `402 Payment Required`   |
| `deepseek-v4-pro:cloud`     | 1,048,576 | low (default), high, max    | `402 Payment Required`   |
| `deepseek-v4.1-flash:cloud` | 1,048,576 | low, high (default), max    | `402 Payment Required`   |
| `glm-5.3:cloud`             | 1,048,576 | low, high, max (default)    | `402 Payment Required`   |
| `glm-5.3-flash:cloud`       | 1,048,576 | low, high, max (default)    | `402 Payment Required`   |
| `minimax-m3:cloud`          | 512,000   | none listed                 | `402 Payment Required`   |

The six that answered `402` are listed as "(paid plan)"; a task on one ends with "This model
needs a paid Ollama plan", never a raw error. Plenipo never changes the plan.

## Built

- [x] Plenipo's Ollama helper (`crates/runtime/src/agent/ollama/bridge.rs`): the sign-in check
      (`/api/me`, plan only) and a task (`/api/chat`, streamed), to `127.0.0.1:11434` only;
      conversation history in the session's folder, trimmed when too long.
- [x] The Ollama adapter (`crates/runtime/src/agent/ollama/mod.rs`): found at
      `%LOCALAPPDATA%\Programs\Ollama\ollama.exe` or on `PATH`; version checked; sign-in shown
      as "Ollama sign-in (free plan)"; `gpt-oss:120b-cloud` with thinking low, medium, and high;
      no variables passed through; conversation only (no tools, no permission grant).
- [x] The runtime runs bridged AI tools through the helper (`AgentConfig::bridge`); the desktop
      app points it at itself and handles `--plenipo-ollama` before any window opens.
- [x] Test double: the `ollama` persona in `plenipo-fake-agent` (its program and the helper).
- [x] Tests: helper against a stand-in service (sign-in, streaming, history, trimming, 401, 429,
      not running), parser, contract suite, runtime harness (runs, resumes, no helper → not
      usable), desktop and workforce counts, end-to-end card.
- [x] On screen: the AI tools page, Workers, and Settings name Ollama; the setup guide has its
      row and notes.

## Owner's check (2026-09-27)

- [x] The Ollama card shows Ready and "Ollama sign-in (free plan)".
- [x] A task and a follow-up answer; a paid-plan model says it needs a paid plan.
- [x] Signing out shows not signed in; signing back in restores it.
- [x] Ollama not running: the card says so.

## Still to check

- The free plan's usage-limit message (seen in use).
- Follow-up: Plenipo's tools for Ollama workers through Guard (ADR-017 §4).
