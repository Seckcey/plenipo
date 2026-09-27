# Ollama (cloud models) — AI tool checklist

**Status:** step 0 (checking the real CLI) in progress on `claude/ai-tools-ollama`.

Adds Ollama's **cloud models** as an AI tool under ADR-014 (the rules for adding AI tools ahead of
Phase 15). The owner has no dedicated graphics card, so Plenipo uses models that run on Ollama's
servers under the owner's ollama.com account (signed in with `ollama signin`; free plan), reached
through the Ollama service on the PC. No local models, no API key. The raw outputs are in
[`crates/runtime/tests/fixtures/ollama-0.34.4/`](../../crates/runtime/tests/fixtures/ollama-0.34.4/README.md).

## Step 0: the bar, checked on the real CLI

| Check                  | Ollama 0.34.4                                                                                                                                                                | Bar item | Result                       |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ---------------------------- |
| Install                | A native `ollama.exe` in `%LOCALAPPDATA%\Programs\Ollama` (the official installer); the Ollama service runs in the background.                                               | 1        | Pass                         |
| Version                | `ollama --version` → `0.34.4`; `GET /api/version` → `{"version":"0.34.4"}`.                                                                                                  | 5        | Pass                         |
| One task, no questions | `ollama run <model>` reads the prompt from stdin and exits by itself. The service's `POST /api/chat` takes the prompt in the request body.                                   | 1        | Pass                         |
| Output                 | The CLI prints plain text with terminal control codes. `POST /api/chat` streams one JSON object per line: thinking, answer, then `done` with `done_reason` and token counts. | 2        | Pass (service)               |
| Resume                 | Ollama keeps no conversations. `POST /api/chat` answered from the earlier messages Plenipo sent, so Plenipo keeps the history itself.                                        | 5        | Pass (Plenipo keeps history) |
| Sign-in status         | `ollama signin` / `signout`; no status command. To check: the service's account endpoint.                                                                                    | 3, 4     | To do                        |
| Errors                 | The free plan's usage-limit text and the signed-out text: to find (docs, or seen in use).                                                                                    | 2        | To do                        |
| Models and effort      | `gpt-oss:120b-cloud`: 117B, 131,072-token context, capabilities completion, tools, thinking; thinking levels low, medium (default), high. Other cloud models: to list.       | —        | Partial                      |
| Least privilege        | A model alone reads no files and runs nothing; it can only answer. Tools exist only if Plenipo offers them (`tools` in `/api/chat`), so Plenipo decides every one.           | —        | Pass                         |
| Credentials in the env | No `OLLAMA_*` variables set; no `OLLAMA_API_KEY`. The service listens on `127.0.0.1:11434` only. The sign-in key lives in `%USERPROFILE%\.ollama` (never read by Plenipo).   | 3        | Pass                         |

## Decisions this needs

A decision record, **ADR-017 (running Ollama's cloud models through its service on this PC)**,
because Ollama differs from the other AI tools in three ways:

- Plenipo sends each task to the Ollama service on `127.0.0.1` instead of starting a program per
  task (ADR-007, how Plenipo runs Claude Code and Codex; ADR-005, the runtime supervisor).
- Plenipo keeps the conversation history and sends it with each task.
- Ollama workers start as conversation only: no files or programs until Plenipo offers its tools
  through Ollama's tool calls, each checked by Guard (ADR-013).

## Still to check

- The tool-call test (it did not run: the model name was empty in that PowerShell window).
- The account endpoint for the sign-in check; `ollama launch --help`.
- The free plan's usage-limit and signed-out texts; the list of cloud models.
  EOF
