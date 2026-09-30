# Phase 16 Wave 2 — the owner's checks on Windows (2026-09-30)

What the owner's PC answered when asked to run GitHub Copilot and Cursor's agent the way Plenipo
would. The steps were given in chat and first tested on the build machine (Linux, PowerShell 7.4,
Copilot 1.0.89 and Cursor's agent 2026.09.28-64d2043, signed out). See the
[Wave 2 checklist](../../../phase-16-wave-2-checklist.md).

**Removed before saving:** the owner's Windows user name (in file paths, and in the pieces of
text a model streamed), and the GitHub account name (the check hid it itself). Screenshots are
described here, not kept: they show the owner's name and billing.

## `copilot/` — GitHub Copilot CLI 1.0.89

Copilot ran with `COPILOT_HOME` pointed at a fresh folder (the way Plenipo runs it), signed in
there with `copilot login`.

| File                            | What it shows                                                                                                                                                                                                                                                                                                               |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `copilot-probe.json`            | `copilot --headless --stdio`: `auth.getStatus` says `isAuthenticated: true`, `authType: "user"`. `account.getQuota` lists `chat` (200), `completions` (2000), and `premium_interactions` (0, `hasQuota: false`), each with `overageAllowedWithExhaustedQuota: false`. `models.list` lists only Auto.                        |
| `copilot-probe-empty-home.json` | The same with an empty settings folder: Copilot fell back to the GitHub CLI's sign-in, `authType: "gh-cli"`, with the same allowances.                                                                                                                                                                                      |
| `copilot-task.jsonl`            | One task, words on standard input, `--output-format json`, tools off: `session.auto_mode_resolved` (Auto chose `mai-code-1.1-flash`), `assistant.message_delta`, `assistant.message` "OK", `session.usage_checkpoint`, and `result` with `sessionId`, `exitCode: 0`, `usage.premiumRequests: 1`.                             |
| `copilot-resume.jsonl`          | `--resume <that sessionId>`: the answer "heron", the same `sessionId`.                                                                                                                                                                                                                                                      |
| `copilot-tool.jsonl`            | Asked to make a file with its tools off: the model wrote a pretend tool request as text (`phase: "commentary"`), no `tool.execution_*` event, and `proof.txt written: False`.                                                                                                                                                |

**GitHub's page** (github.com → Settings → Billing and licensing → Budgets and alerts, the owner's
screenshot): five account budgets; the one for **All AI Credit SKUs** is **$0** with **Stop usage:
Yes**.

## `cursor/` — Cursor's agent

The `agent` command on the owner's PC is xAI's Grok program, not Cursor's, so these files are
Grok's answers. They are kept because they show why Cursor's own program never ran.

| File                 | What it shows                                                                                      |
| -------------------- | -------------------------------------------------------------------------------------------------- |
| `cursor-version.txt` | `agent --version` → `grok 1.0.44 (5b807183dd79) [stable]`.                                          |
| `cursor-models.txt`  | `agent models` → "You are logged in with grok.com.", default `grok-4.7`, four Grok models.          |
| `cursor-status.json` | `agent status --format json` gave nothing Plenipo's step could read (Grok has no such command).    |
| `cursor-task.jsonl`  | `agent -p …` → Grok's own error: "a value is required for '--single <PROMPT>'", exit 2.           |

**The owner's screenshots:**

- `/usage` inside `agent`: Grok's screen, "Weekly limit (SuperGrok)", 0% used.
- cursor.com/dashboard/spending: Cursor **Free** plan ("Free (with SuperGrok)"); "Grok Bot"
  SuperGrok usage 13%; **On-Demand Spending: Disabled**, Monthly Limit: Disabled, with an
  "Enable On-Demand" button ("Enable on-demand billing to keep using Cursor and Grok Bot after
  included usage runs out").
