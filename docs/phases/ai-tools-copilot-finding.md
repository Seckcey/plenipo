# Finding: GitHub Copilot does not meet the bar yet

> **Answered (2026-09-30).** Copilot's second try passes through the two-way route this finding
> names: [ADR-083 (GitHub Copilot as an AI tool, checked before every task)](../adr/ADR-083-github-copilot-as-an-ai-tool.md).
> This page is kept as the record of the first try.

Plenipo does not add GitHub Copilot as an AI tool for now. Copilot's official CLI does the hard
parts well: it takes the prompt on standard input, reports JSON lines, resumes conversations by
ID, and can be run with all its own tools turned off. Two things are missing from its one-task
mode:

1. **A sign-in status check.** There is no command that says whether Copilot is signed in, or how.
2. **A guarantee against paid extra requests.** Once the monthly allowance is used up, GitHub
   may charge for more if the account allows it. The CLI does not ask first, and one-task mode
   gives Plenipo no way to check beforehand.

Both are required by ADR-014 (adding AI tools ahead of Phase 15) and ADR-007 (how Plenipo runs
Claude Code and Codex). Following ADR-014 §4, this branch merges this finding instead of an
adapter. Copilot can be tried again when either answer below changes.

## Tool and version checked

- **GitHub Copilot CLI** (`copilot`), `GitHub Copilot CLI 1.0.88.` The source was GitHub's npm
  package `@github/copilot` 1.0.88 (repository `github/copilot-cli`, build `52f75603`). It was
  installed with `npm install -g @github/copilot@1.0.88` and run on Linux on 2026-09-26.
- **On Windows**, the same npm package puts a real `copilot.exe` in
  `node_modules\@github\copilot\node_modules\@github\copilot-win32-x64\` behind the `copilot.cmd`
  shim. `winget install GitHub.Copilot` gives a native `copilot.exe` too.
- **Nothing was signed in.** Every command ran with a cleared environment, so the container's
  `GH_TOKEN` and `GITHUB_TOKEN` never reached the CLI.
- The captured output is in [`evidence/ai-tools-copilot/`](evidence/ai-tools-copilot/README.md).
  Some runs used the real CLI with a local stand-in model (its offline mode) to see the CLI's
  own behavior without GitHub's service. The evidence page says what those runs can and cannot
  show.

## Bar items that failed

**ADR-014, bar item 4:**

> **A sign-in status check that can tell a subscription from an API key.** This is a command
> Plenipo runs before every turn, as with `claude auth status` and `codex login status`.

The CLI has no such command:

- `copilot login status` is refused ("unexpected argument 'status'").
- `copilot status`, `auth status`, `whoami`, and `user` are not commands.
- Sign-in, sign-out, and the account list live only inside the interactive screen (`/login`,
  `/logout`, `/user`).
- A task that is signed out stops at once with exit code 1 and
  `Error: No authentication information found.` That is a failed task, not a status check.

It matters because Copilot can sign in several ways, and Plenipo may use only the first:

- your own sign-in (`copilot login`);
- the GitHub CLI's stored sign-in;
- a token in `COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, or `GITHUB_TOKEN`;
- a custom model provider with its own API key, from `COPILOT_PROVIDER_*` variables or a
  `providers.json` file in Copilot's folder. It bills that provider pay-per-use, not the
  Copilot plan.

Clearing the environment removes the variables, but not the file. Without a status check,
Plenipo cannot tell which of these a task will use.

**ADR-014, bar item 3, and ADR-007 §4 (billing):**

> **Subscription sign-in only.** … Pay-per-use API billing stays off.

Copilot plans include a monthly allowance, counted in premium requests or, on the newer billing,
AI credits. Past the allowance, GitHub charges for "additional usage" if the account (or its
organization) has a budget for it:

- **Does it tell you before charging?** No. One-task mode has no confirmation. The interactive
  screen shows the remaining allowance in its footer and in `/usage`.
- **How does it report the allowance used up?** As a `session.error` event with
  `errorType: "quota"`, codes `quota_exceeded`, `session_quota_exceeded`, or
  `billing_not_configured`, and exit code 1. This comes from the CLI's own event schema; a real
  refusal was not seen, because nothing was signed in. It happens only when GitHub refuses the
  request, which it does when paid extra usage is off.
- **When paid extra usage is on**, GitHub keeps answering and bills the account. The CLI's
  per-request quota report (`overage`, `overageAllowedWithExhaustedQuota`) is marked internal.
  It did not appear in the one-task JSON output, and it would arrive only after the request was
  made.

So Plenipo can treat "allowance used up" as a usage limit. But in one-task mode it **cannot
guarantee** it never continues into paid requests. That depends on a GitHub account setting
that Plenipo cannot see from one-task mode. `--max-ai-credits` is not a guard: it is a soft cap
on all use, included or paid, and one request can pass it.

## What passed

| Check                                      | Evidence                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1. Official CLI, one task, prompt on stdin | Pipe the prompt in with no `-p`: the CLI runs one task and exits (0 on success). A 280 KB prompt with non-English text, quotes, and shell characters arrived byte for byte. Empty input is refused: "provide a prompt with -p or via standard in". It never stopped to ask.                                                                                                                     |
| 2. Structured output                       | `--output-format json`: one JSON event per line (`assistant.message_delta`, `assistant.message`, `tool.execution_*`, `session.error`, …) and a final `result` line with `sessionId`, `exitCode`, and `usage.premiumRequests`. The events are defined in the CLI's own `session-events.schema.json`.                                                                                             |
| 5. Stable execution                        | `--version`; `--resume <id>` continues a conversation; `--session-id <uuid>` starts one with an ID Plenipo chooses; an unknown ID fails with exit 1.                                                                                                                                                                                                                                            |
| Least privilege                            | `--available-tools=<a name that matches no tool>` leaves the model no tools. A tool call the model asks for anyway is refused ("Tool 'bash' does not exist.") and no file is written. Even without it, one-task mode refuses shell and file changes without asking. `--deny-tool=shell --deny-tool=write --deny-tool=url` also refuses. `--disable-builtin-mcps` drops GitHub's own MCP server. |
| Updates                                    | `--no-auto-update` (or `COPILOT_AUTO_UPDATE=false`). npm installs never update themselves; they only report a newer version.                                                                                                                                                                                                                                                                    |

Not settled, for a later try:

- **Telemetry.** GitHub's usage telemetry has no documented off switch in one-task mode.
  OpenTelemetry export is off by default. `COPILOT_OFFLINE=true` turns everything off, but it
  requires a custom model provider, which is out of scope.
- **The live model list and each model's effort levels.** The CLI gets these from GitHub after
  sign-in. The owner's check lists them.

## What would change the answer

Either of these:

- **GitHub adds a status command to the CLI.** It would need to report the sign-in type and
  whether paid extra usage is on. Or GitHub adds a one-task option that stops at the allowance
  instead of paying. Either brings Copilot back under ADR-014 as written.
- **A new decision record accepts a two-way route for the check before each task.** The route
  already exists: `copilot --headless --stdio`, the JSON-RPC connection GitHub's own
  `@github/copilot-sdk` uses. It answers `auth.getStatus` (signed in or not, and how: `user`,
  `gh-cli`, `env`, `token`, `api-key`) and `account.getQuota` (each allowance, with
  `overageAllowedWithExhaustedQuota`). Signed out it answers cleanly. The tasks themselves
  could stay one-task on standard input. See the
  [notes for that decision record](ai-tools-copilot-decision-notes.md). The
  [owner's check on Windows](ai-tools-copilot-owner-check.md) gets the real, signed-in answers
  to both calls, to confirm the route before anyone builds on it.

## Not done

- **An adapter, a fake-CLI persona, or registration.** Per ADR-014 §4, a tool that fails the bar
  gets a finding, not a workaround.
- **Signing in to GitHub in the capture container.** The owner asked for no sign-in there.
- **Trusting the owner's GitHub budget setting without checking it.** Plenipo cannot see it from
  one-task mode, so it would not be a guarantee.
- **Relying on `--max-ai-credits`.** It is a soft cap on all use, not a stop at the allowance.
- **Passing a token** (`COPILOT_GITHUB_TOKEN`, `GH_TOKEN`, `GITHUB_TOKEN`) or **a custom model
  provider** (`COPILOT_PROVIDER_*`). Both are ruled out by ADR-007 and ADR-014 §7.
- **Driving the interactive screen** (`/usage`, `/user`) or **unofficial clients**: ruled out by
  ADR-014 §7.
