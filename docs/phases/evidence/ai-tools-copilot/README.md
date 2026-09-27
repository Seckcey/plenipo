# GitHub Copilot CLI 1.0.88: captures

Output from GitHub's official Copilot CLI, `@github/copilot` 1.0.88 from npm (repository
`github/copilot-cli`, build `52f75603`). It was installed with `npm install -g @github/copilot@1.0.88`
on Linux on 2026-09-26. These files are the evidence for
[the Copilot finding](../../ai-tools-copilot-finding.md). If Copilot is tried again, they are a
starting point for its adapter's tests and fake persona.

**Nothing here was signed in.** The capture container had `GH_TOKEN` and `GITHUB_TOKEN` set,
so every Copilot command ran with a cleared environment (`env -i`, then only `HOME`, `PATH`,
`COPILOT_HOME`, and the proxy and certificate settings). This is the same way Plenipo starts an
AI tool. No token variable reached the CLI, and no `copilot login` was run.

**Redacted:** the capture folders are now `/work` (the task's folder), `/home/owner` (home), and
`/home/owner/.copilot` (Copilot's folder). The npm install folder is now `/usr/lib/node_modules`.
Session IDs are the random ones the CLI made, except the one Plenipo-style chosen ID
`00000000-0000-4000-8000-000000000001`.

## Real CLI, signed out

| File                           | Command                                                                                                                                                                       | Exit |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---- |
| `version.txt`                  | `copilot --version`                                                                                                                                                           | 0    |
| `help/*.txt`                   | `copilot --help`, `copilot help <topic>`, `copilot <command> --help`                                                                                                          | 0    |
| `turn-signed-out.stderr.txt`   | `copilot --output-format json --no-auto-update` with a prompt piped in; no stdout                                                                                             | 1    |
| `turn-empty-stdin.stderr.txt`  | the same with empty standard input                                                                                                                                            | 1    |
| `headless-rpc-signed-out.txt`  | `copilot --headless --stdio`: `connect`, `status.get`, `auth.getStatus`, `account.getQuota`, `models.list` over JSON-RPC (the connection GitHub's `@github/copilot-sdk` uses) | —    |
| `headless-rpc-close-early.txt` | the same requests written at once with standard input closed straight away: answers are sometimes dropped                                                                     | 0    |
| `acp-signed-out.txt`           | `copilot --acp`: `initialize`, then `session/new`                                                                                                                             | 0    |
| `env-vars.txt`                 | every `COPILOT_*`, `GH_*`, and `GITHUB_*` name in the CLI's own code (names only)                                                                                             | —    |
| `win32-x64-package.txt`        | the files in `@github/copilot-win32-x64@1.0.88` (the Windows build npm installs)                                                                                              | —    |

## Real CLI, stand-in model (`stand-in/`)

GitHub's service needs a signed-in account, so these runs used the real CLI in its **offline
mode**. That mode points it at a local stand-in model server on `127.0.0.1` through
`COPILOT_OFFLINE=true`, `COPILOT_PROVIDER_BASE_URL`, and `COPILOT_MODEL=gpt-5.4`. The stand-in
logged what the CLI sent and answered with fixed text such as
`pong (user messages seen: 2; tools offered: 0)`. When the prompt asked it to, it requested a
tool call or returned an HTTP error. These runs show the CLI's own behavior: standard input,
JSON lines, resume, tool refusal, effort, and error events. They say **nothing** about GitHub's
service: its sign-in, its allowance, its model list, or its exact error wording. Plenipo would
never use this mode, because a custom model provider is pay-per-use.

The runs used this flag set unless the file says otherwise:
`--output-format json --no-auto-update --available-tools=plenipo_no_tools --disable-builtin-mcps --no-ask-user`.

| File                           | What it shows                                                                                                                                             |
| ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tools-offered.txt`            | The tools the CLI offered the model: 16 by default (`bash`, `apply_patch`, `view`, `rg`, `glob`, `task`, …), 0 with `--available-tools=plenipo_no_tools`. |
| `turn-new.jsonl`               | A whole task: prompt on stdin, one JSON event per line, final `result` line with `sessionId`, `exitCode`, and `usage.premiumRequests`. Exit 0.            |
| `turn-resume.jsonl`            | `--resume <sessionId>`: the model saw both messages. Exit 0.                                                                                              |
| `turn-chosen-session-id.jsonl` | `--session-id <uuid>` on a new conversation: the CLI used the chosen ID. Exit 0.                                                                          |
| `turn-resume-unknown.txt`      | `--resume` with an ID that does not exist. Exit 1.                                                                                                        |
| `tool-default-one-shot.jsonl`  | The model asked to run `touch …` with default flags: refused, "Permission denied and could not request permission from user". No file was made. Exit 0.   |
| `tool-tools-hidden.jsonl`      | The same with `--available-tools=plenipo_no_tools`: "Tool 'bash' does not exist." No file was made. Exit 0.                                               |
| `tool-deny-rules.jsonl`        | The same with `--deny-tool=shell --deny-tool=write --deny-tool=url`: refused by the rule. No file was made. Exit 0.                                       |
| `effort.txt`                   | `--reasoning-effort` takes `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, `max` and passes it on; anything else exits 1 before running.             |
| `error-402.jsonl`              | The stand-in answered HTTP 402: `session.error` (`errorType` `query` for a custom provider), a `result` line with `exitCode` 1. Exit 1.                   |
| `error-401.jsonl`              | HTTP 401: `session.error` with `errorType` `authentication`. Exit 1.                                                                                      |
| `error-429.jsonl`              | HTTP 429 with `retry-after: 60`: the CLI waits and retries on its own. It was stopped by a 45-second time limit (exit 124).                               |
