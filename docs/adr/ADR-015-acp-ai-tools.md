# ADR-015: Running AI tools over ACP when their one-task mode cannot read standard input

- **Status:** Accepted (by the owner, 2026-09-26)
- **Date:** 2026-09-26
- **Phase:** 15 (adapter parts pulled forward, as in ADR-014)

> **On screen** (ADR-010, plain words and rank names): nothing new. ACP is how Plenipo talks to
> the AI tool behind the scenes; the owner still sees "AI tool", "conversation", and "task".

## Context

ADR-014 (adding AI tools ahead of Phase 15) lets Plenipo add an AI tool only if its official CLI
"runs one task and exits by itself, reads the prompt from stdin, and never stops to ask a
question" (bar item 1). The stdin rule comes from ADR-007 (how Plenipo runs Claude Code and
Codex): a prompt on the command line shows in program listings, is length-limited on Windows,
and invites quoting mistakes.

xAI's Grok CLI (`grok 1.0.41`, and the `1.0.42` alpha) fails that item. Its one-task mode,
`grok -p`, takes the prompt only as an argument or a file, and xAI's guide says "Headless mode
does not read piped stdin into the prompt"
([finding](../phases/ai-tools-grok-finding.md)). The owner wants Grok, and has ruled out both
the prompt on the command line and the prompt in a file.

The same CLI has a second official mode, `grok agent stdio`, which speaks **ACP** (Agent Client
Protocol). ACP is an open protocol for apps that drive AI coding tools; the Zed, Neovim, and
Emacs editors use it. Messages are JSON-RPC 2.0, one per line, on the program's standard input
and output, so the prompt goes in on standard input. Google's Gemini CLI speaks it too
(`gemini --acp`, checked on 0.61.0).

ACP is not one-way. Today a task is a handoff: Plenipo writes the whole prompt to the program's
standard input, closes it, and reads events until the program exits
(`crates/runtime/src/agent/adapter.rs`: the adapter builds arguments, the parser only reads).
In ACP, Plenipo and the AI tool exchange messages during the task:

1. Plenipo: `initialize` (what each side supports). The tool answers with its models, effort
   levels, and sign-in methods.
2. Plenipo: `session/new` (working folder, tool servers), or `session/load` / `session/resume`
   for a follow-up. The tool answers with the conversation ID. **The prompt needs that ID**, so
   Plenipo must wait for this answer before sending it.
3. Plenipo: `session/prompt` with the task text. The tool streams `session/update`
   notifications (its text, thinking, tool use, plan) and may ask
   `session/request_permission` before using a tool. **Plenipo must answer each request.**
4. The tool answers `session/prompt` with a stop reason. Plenipo closes the program's input and
   the program exits.

What was verified on the real CLI, signed out (evidence in
[`../phases/evidence/ai-tools-grok/`](../phases/evidence/ai-tools-grok/README.md)): `initialize`
answers with protocol version 1, the model menu with each model's effort levels, one sign-in
method ("Sign in with Grok"), and support for loading and resuming conversations. `session/new`
signed out answers with a plain error (`-32000 Authentication required`) and no browser prompt.
Nothing signed in has been seen yet.

ADR-014 §7 says a tool that needs more than its contract allows needs its own decision record.
This is that record.

## Decision

1. **A second way to run a task: over ACP.** An adapter runs its tasks one of two ways:
   - **One-way** (unchanged): the prompt on standard input, then output until exit. Claude Code
     and Codex stay this way.
   - **ACP**: the prompt and everything else as ACP messages on standard input. Used only when
     the AI tool's one-task mode cannot read the prompt from standard input. Grok is the first.
     Gemini's branch decides for Gemini: its `-p` help says it reads standard input, and if that
     holds it stays one-way.

   This widens ADR-014's bar item 1 to: "runs one task and exits by itself, never stops to ask a
   question, and reads the prompt from stdin — directly, or over ACP (ADR-015)." Every other bar
   item, and ADR-014's out-of-scope list, stay as they are.

2. **Still one supervised program per task.** Each task starts the AI tool's ACP mode as its own
   supervised program (ADR-005, the Runtime Supervisor decision): its own program tree, a time
   limit, cancel, shutdown handling, and the Ledger record. The program ends when the task ends.
   Plenipo does not keep an AI tool running between tasks, and does not use an AI tool's shared
   background process (Grok's "leader": `--no-leader`), which would run outside Plenipo's
   supervision.

3. **The contract change.** The supervisor can keep a task's standard input open, and the task's
   parser can write to it:
   - the parser gets the prompt and returns the first messages to send (`initialize`, then
     `session/new` or `session/load`/`session/resume`);
   - each output line can return messages to send as well as events (the prompt after the
     conversation ID arrives; answers to permission requests);
   - the parser says when the task is over, and the supervisor then closes standard input.

   One-way adapters do not change: they send nothing after the prompt. The prompt still never
   appears in the program's arguments.

4. **One shared ACP driver.** A single module (`crates/runtime/src/agent/acp.rs`) speaks ACP for
   every adapter that uses it. An ACP adapter supplies only what differs: the executable, the
   launch arguments, the sign-in check, the variables, the least-privilege setup, and any
   tool-specific extras. The messages are written with `serde_json`, which Plenipo already uses;
   no new dependency. Unknown messages and notifications are ignored and counted, as ADR-007 §7
   requires. Plenipo speaks ACP protocol version 1 and stops the task if the tool answers with
   another.

5. **Least privilege and Guard.** Plenipo tells the tool it offers no file or terminal access of
   its own (`fs` and `terminal` off in `initialize`). A worker with permissions gets Plenipo's
   tool server (ADR-013, Guard and the capability broker) in `session/new`'s tool-server list;
   ACP takes the same program-plus-arguments the server already has. The AI tool's own built-in
   tools are switched off as far as the tool allows; for Grok, a per-conversation agent profile
   (`--agent-profile`) that allows no built-in tools, plus the documented switches that turn off
   subagents, memory, and web fetch (`GROK_SUBAGENTS=0`, `GROK_MEMORY=0`, `GROK_WEB_FETCH=0`).
   Grok also reads the owner's Claude Code and Cursor skills, hooks, and tool servers; the
   switches for that (`GROK_CLAUDE_*_ENABLED`, `GROK_CURSOR_*_ENABLED`) are in the program but
   not in xAI's docs, so the Grok branch confirms them on the real CLI. Plenipo answers
   every permission request itself, never the owner:
   - a call to Plenipo's tool server: allowed (Guard decides inside the call, as for Claude Code
     and Codex);
   - anything else: refused.

   Grok's kernel sandbox (`--sandbox`) works on Linux and macOS only; on Windows these
   measures are the limit. If a signed-in check shows Grok still runs a built-in tool without
   asking, the Grok branch stops and asks the owner (ADR-014 §5).

6. **Sign-in and billing (ADR-007 §4, unchanged).**
   - Plenipo runs the tool's sign-in status check before every task, with the task's own
     variables. For Grok that is `grok models`, whose first line names the credential.
   - Plenipo never passes API-key or billing variables (for Grok: `XAI_API_KEY`,
     `GROK_CODE_XAI_API_KEY`, `GROK_DEPLOYMENT_KEY`, and the auth-provider, OIDC, and endpoint
     variables).
   - Plenipo turns the tool's own API-key sign-in off when it can. For Grok,
     `GROK_DISABLE_API_KEY_AUTH=1` on every launch; a task with an API key then stops at "Not
     signed in" (tested with a fake key).
   - Grok lets a key set on a model in its own settings file win over the sign-in. The Grok
     adapter is not merged until the owner's Windows check shows either that the switch above
     blocks such a key, or how a signed-in task reports the credential it used. If neither
     holds, the Grok branch stops and asks the owner.

7. **Conversations, cancel, and results.**
   - The conversation ID is the one the tool returns from `session/new` (the tool picks it,
     not Plenipo). A follow-up uses `session/resume` when the tool offers it, otherwise
     `session/load`; history the tool replays while loading is not recorded again.
   - Model and effort are launch options (`-m`, `--reasoning-effort` for Grok). The model
     lists stay declared in `capabilities()` (ADR-014 §6); the `initialize` answer is used to
     notice when they are out of date, not to replace them.
   - Cancel sends `session/cancel`, waits briefly, then ends the program tree as today.
   - The final answer is the text the tool streamed for the prompt; usage comes from the
     `session/prompt` answer. Usage limits and sign-in errors map to ADR-007's outcomes from the
     texts the owner's check records.

8. **Tests.** `plenipo-fake-agent` gains an ACP persona that answers these messages, with its
   markers for errors, permission requests, and usage limits. The contract suite
   (`crates/runtime/tests/contract.rs`) covers ACP adapters like any other. The shared driver
   has its own unit tests, including answering a permission request and ignoring unknown
   messages.

## Consequences

- Grok can be added, over ACP, once the owner's signed-in check passes. Gemini, and any later
  tool that speaks ACP, can reuse the driver.
- Plenipo has two ways to run a task to maintain. The one-way path does not change, and the ACP
  path is one module.
- The supervisor and the task parser gain a way to write during a task. This touches the same
  code as ADR-013 (Guard), which is now merged.
- ACP permission requests become a second place where Plenipo refuses an AI tool's own tools,
  alongside Guard; both are logged in the task's activity.
- ACP is young and has "unstable" parts; Grok adds its own `x.ai/` messages. Plenipo uses only
  the stable core above, parses defensively, and records the CLI version, as ADR-007 already
  requires.
- When xAI lets `grok -p` read standard input, Grok can move to the one-way path; this decision
  does not require ACP where one-way works.

## Alternatives considered

- **Prompt on the command line** (`grok -p "<prompt>"`): ruled out by ADR-007 and ADR-014.
- **Prompt in a file** (`--prompt-file`): not standard input; ruled out by the owner.
- **`--prompt-file /dev/stdin`**: Linux and macOS only; Windows is Plenipo's target.
- **Keep one ACP program running per conversation**: faster follow-ups, but a long-lived program
  outside the per-task supervision, time limit, and shutdown handling of ADR-005.
- **Grok's shared background process ("leader")**: runs outside Plenipo's supervision.
- **The `agent-client-protocol` Rust crate**: works, but adds a dependency for a handful of
  messages Plenipo can write with `serde_json`.
- **Wait for xAI to add standard input to `-p`**: no Grok until then, with no date.
