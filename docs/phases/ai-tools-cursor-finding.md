# Finding: Cursor's agent does not meet the bar yet

Plenipo does not add Cursor's agent as an AI tool for now. Its command-line program does most of
the hard parts: it reads the task from standard input, reports JSON lines, continues a chat by its
ID, and has a sign-in check. One thing is missing: **nothing a program can run says whether
Cursor may charge for paid "on-demand" use** once the plan's included usage runs out. That setting
shows only on Cursor's website and on the program's own screen.

This is required by ADR-014 (adding AI tools) bar item 3, "subscription sign-in only … pay-per-use
billing stays off", and by ADR-036 (every AI model worth having): nothing paid before spending
caps. Following ADR-014 §4, Phase 16 Wave 2 merges this finding instead of an adapter; the
decision is [ADR-084 (Cursor's agent waits)](../adr/ADR-084-cursor-agent-waits.md). Cursor's agent
can be tried again when the answer below changes.

## Tool and version checked

- **Cursor Agent** (`cursor-agent`, also installed as `agent`), version `2026.09.28-64d2043`, from
  Cursor's own installer (`https://cursor.com/install`, which downloads
  `downloads.cursor.com/lab/2026.09.28-64d2043/<os>/<arch>/agent-cli-package`). Checked on Linux
  on 2026-09-30, **signed out**, with a cleared environment (no key or token reached it).
- **On Windows**, Cursor's installer (`irm 'https://cursor.com/install?win32=true' | iex`) puts
  the program in `%LOCALAPPDATA%\cursor-agent\` as `cursor-agent.cmd` / `agent.cmd` shims that run
  PowerShell, which runs the bundled `node.exe` with `index.js`.
- **On the owner's PC**, the `agent` command is xAI's Grok program (`grok 1.0.44`), so Cursor's own
  program did not run there. The owner's Cursor account (its website, 2026-09-30): the **Free**
  plan, linked to SuperGrok, with **On-Demand Spending: Disabled**. The owner: "Grok is cursor."
  See [the owner's check](evidence/phase-16-wave-2/owner-check/README.md).

## Bar item that failed

**ADR-014, bar item 3, and ADR-007 §4 (billing):**

> **Subscription sign-in only.** … Pay-per-use API billing stays off.

Cursor's plans include some usage. Past it, Cursor keeps answering and bills the account if
**on-demand usage** is on ("Enable on-demand billing to keep using Cursor and Grok Bot after
included usage runs out", its website). Plenipo must be sure, before every task, that it is off.
It cannot be:

- **`agent status --format json`** says only whether it is signed in: `status`,
  `isAuthenticated`, `hasAccessToken`, `hasRefreshToken`, `message`, and the account's name and
  team when signed in.
- **`agent about --format json`** adds the plan's name (`subscriptionTier`), and nothing about
  on-demand use.
- **`agent models`**, **`--list-models`**, and **`create-chat`** say nothing about it.
- **Its ACP mode** (`agent acp`, hidden from its help) offers one sign-in method and its own
  commands (`cursor/…`); none reports usage or spending.
- **A task's JSON lines** (`system`/`init`, `assistant`, `tool_call`, `result`) report the model,
  `apiKeySource` (`login`, `env`, or `flag`), and token counts, never whether on-demand use is on,
  and only after the request was made.
- **Only its own screen** reads it: the `/usage` command ("Show plan and on-demand usage") calls
  Cursor's dashboard service (`GetHardLimit`, `GetCurrentPeriodUsage`, `GetPlanInfo`) and shows
  "On-Demand: disabled", a fixed limit, or unlimited. Driving that screen is ruled out by ADR-014
  §7, and calling Cursor's service directly would make Plenipo an unofficial client.

A second, smaller gap: an API key (`--api-key`, `CURSOR_API_KEY`) is traded for the same kind of
stored sign-in as `agent login`, so `status` cannot tell a key from a sign-in. Plenipo could work
around that (it never passes a key, and would keep Cursor's sign-in in a folder of its own), but
the missing on-demand check is enough on its own.

## What passed

| Check                                 | Evidence (build machine, signed out, and the program's own code)                                                                                                   |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1. One task, words on standard input  | `agent -p --output-format stream-json` with no words on the command line reads them from standard input (its `build-prompt` code); it exits by itself.             |
| 2. Structured output                  | `--output-format stream-json`: `system`/`init` (the chat ID, model, `apiKeySource`), `assistant`, `tool_call` started and completed, `thinking`, and one `result`. |
| 4. A sign-in check (signed in or not) | `agent status --format json`: `{"status":"unauthenticated","isAuthenticated":false,…}` signed out. Signed out, a task stops: "Authentication required".            |
| 5. Stable execution                   | `--version` (`2026.09.28-64d2043`); `--resume <chatId>` and `create-chat`; `update`.                                                                               |
| Least privilege                       | `--mode ask` or `--mode plan` (read-only); its permission rules in its settings file (`cli-config.json`), which a settings folder of its own could hold.           |

## What would change the answer

Either of these:

- **Cursor adds a way for a program to see on-demand use**: a field in `agent status` or
  `agent about`, or a flag that makes a task stop at the included usage instead of paying.
- **Cursor documents a guarantee** that a plan with on-demand use disabled can never be charged
  for a task started from its command-line program, with a way to confirm the setting.

Until then, **Grok** stays Plenipo's AI tool for xAI's models
([ADR-015 (running AI tools over ACP)](../adr/ADR-015-acp-ai-tools.md)), and Anthropic's,
OpenAI's, Google's, and Moonshot AI's models are reached through their own AI tools.

## Not done

- **An adapter, a fake-program persona, or registration.** Per ADR-014 §4, a tool that fails the
  bar gets a finding, not a workaround.
- **Trusting the owner's on-demand setting without checking it.** It is off today, but Plenipo
  could not see if it were turned on.
- **Driving Cursor's own screen** (`/usage`), or **calling Cursor's service** with its stored
  sign-in: ruled out by ADR-014 §7.
- **Passing a key** (`--api-key`, `CURSOR_API_KEY`, `CURSOR_AUTH_TOKEN`), **AWS Bedrock mode**
  (`agent bedrock`), or **Cursor's cloud workers** (`agent worker`): ruled out by ADR-007,
  ADR-014 §7, and Phase 16's Out of Scope.
