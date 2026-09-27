# Ollama (cloud models) — AI tool checklist

**Status:** step 0 (checking the real CLI) in progress on `claude/ai-tools-ollama`.

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

- **ADR-017 (running Ollama's cloud models through the Ollama service on this PC)**, Proposed:
  Plenipo sends each task to the service on `127.0.0.1`, checks the sign-in with `/api/me`, runs
  only cloud models, keeps the conversation history itself, and starts Ollama workers with no
  tools (Plenipo's tools through Guard come as a follow-up).

## Owner's model choices

`gpt-oss:120b-cloud` (checked), deepseek-v4.1-flash, glm-5.3, glm-5.3-flash, minimax-m3,
deepseek-v4-pro, nemotron-3-ultra (exact names checked when the adapter is built).

## Still to check

- The free plan's usage-limit message (seen in use).
