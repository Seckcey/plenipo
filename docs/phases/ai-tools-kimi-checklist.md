# Kimi (Moonshot AI) — AI tool checklist

**Status:** step 0 (checking the real CLI) in progress on `claude/ai-tools-kimi`.

Adds Kimi Code, Moonshot AI's official coding CLI, as an AI tool under ADR-014 (the rules for
adding AI tools ahead of Phase 15). The owner checked the real CLI on Windows 11 with a Kimi
subscription. The raw outputs are in
[`crates/runtime/tests/fixtures/kimi-0.34.0/`](../../crates/runtime/tests/fixtures/kimi-0.34.0/README.md).

## Step 0: the bar, checked on the real CLI

| Check                  | Kimi Code 0.34.0                                                                                                                                                                                                                                                                                         | Bar item | Result                     |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | -------------------------- |
| Install                | A native `kimi.exe` in `%USERPROFILE%\.kimi-code\bin` (the official installer).                                                                                                                                                                                                                          | 1        | Pass                       |
| Version                | `kimi --version` → `0.34.0`; ACP `initialize` also reports `agentInfo.version`.                                                                                                                                                                                                                          | 5        | Pass                       |
| One task, no questions | Prompt mode (`kimi -p "…" --output-format stream-json`) runs one task and exits, but takes the prompt only as an argument (`-p -` sends the text `-`). **ACP (`kimi acp`) takes the prompt as a JSON-RPC message on stdin**, one process per turn.                                                       | 1, 2     | Pass (ACP)                 |
| Output                 | Prompt mode: one JSON object per line (version, answer, tool calls, session ID). ACP: JSON-RPC over stdio with streamed `session/update` messages (thoughts, answer chunks, tool calls, usage) and `stopReason` at the end.                                                                              | 2        | Pass                       |
| Resume                 | Prompt mode: `-S <id>`. ACP: `session/load` in a new process replays the conversation; the next prompt remembered it.                                                                                                                                                                                    | 5        | Pass                       |
| Sign-in status         | No status command. `kimi provider list` names each provider and its credential source (`managed:kimi-code type=kimi … source=oauth` = the Kimi subscription). ACP `initialize` lists `authMethods` (`login`, device code). Signed-out output: to capture.                                                | 3, 4     | Partial                    |
| Errors                 | Unknown model: `Model "…" is not configured in config.toml.` Usage limit and expired sign-in: to capture (docs or source).                                                                                                                                                                               | 2        | To do                      |
| Models and effort      | ACP `session/new` lists them: `kimi-code/kimi-for-coding` (K2.8 Preview), `kimi-code/kimi-for-coding-highspeed` (K2.7 Code Highspeed), `kimi-code/k3` (K3, default), `kimi-code/k3-256k` (K3-256k); thinking `low`, `high`, `max`.                                                                       | —        | Pass                       |
| Least privilege        | **Prompt mode wrote a file without asking**, and `--plan` cannot be combined with `-p`. In ACP (mode `default`, "manual approvals") Kimi asked permission before writing; the client refused and nothing was written. Modes: `default`, `plan` (read-only), `auto`, `yolo`. Reads and commands: round 4. | —        | Pass (ACP), checking reads |
| Credentials in the env | No `KIMI*` or `MOONSHOT*` variables set; the subscription token lives in `%USERPROFILE%\.kimi-code` (never read by Plenipo).                                                                                                                                                                             | 3        | Pass so far                |

## Decisions this needs

- **Kimi runs through ACP (Agent Client Protocol), not prompt mode.** ADR-007 (how Plenipo runs
  Claude Code and Codex) assumes a turn is "arguments + prompt on stdin + read the output". ACP
  is a conversation over stdin/stdout: Plenipo sends the prompt as a message and answers Kimi's
  permission requests during the turn. Grok's branch reached the same conclusion
  (`grok agent stdio`). This goes in a decision record before adapter code.
- **Plenipo answers Kimi's permission requests with Guard**, the same rules and approval cards as
  Phase 7, and can offer Kimi file access through Plenipo (ACP `fs/*`) so reads stay in the
  project folder too. Round 4 checks whether Kimi uses it.

## Still to check

- Round 4: reads inside and outside the folder, a command, and switching the model, with Plenipo
  offering file access (`kimi-acp-check2.ps1`).
- Signed-out behavior (`session/new` or a prompt while signed out) and the texts of a usage limit
  and an expired sign-in.
