# ADR-015: AI tools connected through ACP, with Plenipo answering their permission requests

- **Status:** Proposed
- **Date:** 2026-09-26
- **Phase:** 15 (adapter parts pulled forward, after v0.8.0)

> **On screen** (ADR-010, plain words and rank names): the owner sees an AI tool ("Kimi") and
> permissions, never "ACP", "session/load", or "JSON-RPC". This ADR keeps the protocol's words.

## Context

ADR-007 (how Plenipo runs Claude Code and Codex) defines a turn as one supervised process: Plenipo
builds the arguments, writes the prompt to stdin, closes stdin, and reads JSON lines until the
process exits. The runtime's supervisor does exactly that: it writes the prompt once and shuts
stdin (`crates/runtime/src/supervisor.rs`).

Checking Kimi Code 0.34.0 on the owner's PC (ADR-014 step 0, the rules for adding AI tools;
evidence in `crates/runtime/tests/fixtures/kimi-0.34.0/`) showed that this shape does not fit
every tool:

- Kimi's one-shot mode (`kimi -p`) takes the prompt only as an argument (`-p -` sends the text
  `-`), and **it wrote a file without asking anyone**. Its read-only `--plan` cannot be combined
  with `-p`.
- Kimi's **ACP** mode (`kimi acp`, the Agent Client Protocol: JSON-RPC 2.0, one message per line,
  over stdin and stdout) fits the rules and more:
  - The prompt is a message on stdin.
  - Before writing a file or running a command, Kimi sends `session/request_permission` and waits.
    A refusal was respected: nothing was written or run.
  - When the client says it can read and write files (`clientCapabilities.fs`), Kimi asks the
    client to read every file (`fs/read_text_file`), inside the folder or not. A refused read
    stays refused, and Kimi said it would not work around it.
  - `session/new` lists the models and thinking levels; `session/set_config_option` switches them.
  - `session/load` reopens a conversation in a new process.
  - `initialize` reports the version and how to sign in; the credential in use shows in
    `kimi provider list` (`source=oauth` for the Kimi subscription).

Grok's CLI reached the same conclusion on its branch (`grok agent stdio` is ACP). More tools are
likely to offer ACP, since it is the protocol code editors use to host coding agents.

## Decision

1. **An AI tool may be connected through ACP** when its one-shot mode fails ADR-014's bar and its
   ACP mode passes it. The tool's adapter says which it uses. Claude Code and Codex keep their
   current one-shot turns.
2. **Still one supervised process per turn.** An ACP turn starts the tool (`kimi acp`) under the
   supervisor as today, then, over stdin and stdout:
   1. `initialize`, offering file access (`fs.readTextFile`, `fs.writeTextFile`) and no terminal;
   2. `session/new` in the turn's folder, or `session/load` to continue a conversation;
   3. the model and thinking level with `session/set_config_option`, and the mode `default`
      (the tool asks before acting);
   4. `session/prompt` with the objective, reading `session/update` messages until the prompt's
      `stopReason`;
   5. closing stdin; the process exits and the turn ends as today.

   The supervisor gains a turn whose stdin stays open, so the adapter can send messages during
   the turn. Time limits, cancelling, process-tree ownership, and restart handling are unchanged.

3. **Plenipo answers every request the tool makes, through Guard** (ADR-013, how Plenipo lets
   workers use your computer safely):
   - `fs/read_text_file` and `fs/write_text_file` are file reads and writes under the worker's
     permissions: inside the project folder, not a blocked file, redacted, recorded, with Ask-me
     approvals as for Plenipo's own tools. A worker with no folder gets every file request refused.
   - `session/request_permission` is answered from Guard's decision on what the tool wants to do
     (its title, kind, and input). Changing files follows the file permissions. **The tool's own
     shell commands are refused**, with a note to use Plenipo's `run_command` tool, so every
     program still runs through Plenipo's supervisor and command rules. Anything Plenipo cannot
     classify is refused.
   - Plenipo never chooses "approve for this session" (`allow_always`): each approval covers one
     action, as in ADR-013.
   - Plenipo's own tools reach the tool as an MCP server in `session/new` (`mcpServers`), the same
     relay and per-step ticket as Claude Code and Codex.
   - Mode `auto` or `yolo` is never used.
4. **Sign-in and billing.** Before a turn, the adapter checks the tool's own account listing (for
   Kimi, `kimi provider list`) and runs only models of the subscription provider (for Kimi,
   `managed:kimi-code`, `source=oauth`). A tool that reports an API key, or an authentication
   error during `initialize` or `session/new`, ends the turn as "sign-in required" or "API billing
   not allowed", as today. API-billed use waits for its own decision record.
5. **Output.** `agent_message_chunk` becomes the answer, `agent_thought_chunk` the reasoning,
   `tool_call` and `tool_call_update` the activity, `usage_update` the usage, and the prompt's
   `stopReason` the outcome. A malformed line is ignored and counted, as for every adapter.

## Consequences

- Tools that connect through ACP are held more tightly than Claude Code and Codex. Their reads
  also go through Guard, which closes, for them, the gap Codex still has (ADR-013: Codex's own
  read-only commands can read outside the folder).
- The runtime supports two turn shapes, one-shot and ACP. The ACP client is written once and
  shared by every ACP adapter (Kimi first, then Grok). The contract suite (ADR-014) gains ACP
  checks, and the fake CLI gains an ACP persona that replays the captured Kimi messages.
- A tool may add request types Plenipo does not know. Unknown requests are refused, and the
  adapter's checked version (`checked_version()`) records what was tested.
- The tool still loads its owner's personal settings (for example Kimi's skills and
  `AGENTS.md`). Their files are read through Plenipo, so the folder rules apply, but a worker may
  see skills the owner installed for their own use of the tool.
- Owner checks with the real tool stay necessary for each new version.

## Alternatives considered

- **One-shot mode (`kimi -p`).** Fails two rules: the prompt must be an argument, and the tool
  acts without asking.
- **Running Kimi read-only (`plan` mode) with only Plenipo's MCP tools.** Workable for roles
  without file permissions, and used for them. For roles with permissions it would still leave
  reads outside Guard, and ACP file access already routes them through Plenipo.
- **Letting the tool run approved shell commands itself.** Fewer round trips, but the command
  would run outside Plenipo's command rules and program log. Refused in favor of Plenipo's
  `run_command`.
- **A long-lived ACP process shared across turns.** Faster starts, but it breaks one process per
  turn, which the supervisor, cancelling, and restart handling rely on. Revisit if start-up time
  matters.
