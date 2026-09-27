# Kimi (Moonshot AI) — AI tool checklist

**Status:** step 0 (checking the real CLI) done on `claude/ai-tools-kimi`: Kimi passes through ACP.
ADR-018 (Kimi over ACP, with its file reads and writes going through Plenipo) accepted by the owner
on 2026-09-26. The adapter waits for the Grok branch, which brings ADR-015 (running AI tools over
ACP) and the shared ACP driver.

Adds Kimi Code, Moonshot AI's official coding CLI, as an AI tool under ADR-014 (the rules for
adding AI tools ahead of Phase 15). The owner checked the real CLI on Windows 11 with a Kimi
subscription. The raw outputs are in
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

## Decisions

- **ADR-015 (running AI tools over ACP)**, from the Grok branch: one supervised program per task,
  the prompt as a message on stdin, one shared ACP driver.
- **ADR-018 (Kimi over ACP, with its file reads and writes going through Plenipo)**, accepted:
  Kimi's built-in tools cannot be switched off, so Plenipo offers file access and answers every
  file request through Guard; Kimi's own shell commands are refused in favor of Plenipo's
  `run_command`; model and thinking level are set with `session/set_config_option`; only the Kimi
  subscription provider (`managed:kimi-code`, `source=oauth`) is used.

## Next

1. The Grok branch merges (ADR-015 and the shared ACP driver).
2. Merge `main` into this branch, then build the Kimi adapter on the driver, with the two driver
   options ADR-018 adds, a Kimi ACP persona in the fake CLI, and tests.
3. The owner's final check with the real Kimi on Windows.

## Still to check

- The texts of a usage limit and an expired sign-in (from Kimi's docs or source, or seen in use).
- `kimi provider list --json`, to parse the credential source reliably (the owner checks it holds
  no token before sharing it).
