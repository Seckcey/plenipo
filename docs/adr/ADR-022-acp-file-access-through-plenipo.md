# ADR-022: Kimi over ACP, with its file reads and writes going through Plenipo

- **Status:** Accepted (by the owner, 2026-09-26)
- **Date:** 2026-09-26 (numbered ADR-016 when accepted; renumbered ADR-022 on 2026-09-27
  because `main` already uses ADR-016 for the Development department, ADR-018 to ADR-020 for
  Phase 9 and Phase 10, and ADR-021 for the editions and license)
- **Phase:** 15 (adapter parts pulled forward, after v0.8.0)

> **On screen** (ADR-010, plain words and rank names): nothing new. The owner sees "Kimi", its
> permissions, and approval cards, never "ACP" or "fs/read_text_file".

## Context

ADR-015 (running AI tools over ACP, from the Grok branch) lets an AI tool whose one-task mode
cannot read the prompt from stdin run over ACP, through one shared driver
(`crates/runtime/src/agent/acp.rs`), one supervised program per task. Its §5 tells the tool that
Plenipo offers no file or terminal access, switches the tool's own built-in tools off as far as
the tool allows, and refuses every permission request that is not a call to Plenipo's tool
server.

Kimi Code 0.34.0, checked on the owner's PC (ADR-014 step 0, the rules for adding AI tools;
evidence in `crates/runtime/tests/fixtures/kimi-0.34.0/`), qualifies for ACP the same way as
Grok:

- `kimi -p` takes the prompt only as an argument (`-p -` sends the text `-`), and in that mode
  **it wrote a file without asking anyone**; `--plan` cannot be combined with `-p`.
- `kimi acp` takes the prompt as a message, and `session/load` reopens a conversation.

Two things differ from Grok, and ADR-015 does not cover them:

1. **Kimi's built-in tools cannot be switched off.** `kimi acp` takes no options (only
   `--login`), and Kimi has no documented switch for its tools. What Kimi does offer, tested:
   - With the client offering file access (`clientCapabilities.fs`), **every** file read came to
     the client as `fs/read_text_file`: a file in the folder, a file outside it, and Kimi's own
     `AGENTS.md` lookups. Refused reads stayed refused, and Kimi said it would not work around
     them.
   - A write (`Write`) and a command (`Bash`) each waited for `session/request_permission`, and a
     refusal was respected: nothing was written or run.
   - Reads with file access off were not tested; Kimi's mode `default` is "manual approvals",
     and a read may not ask.
2. **Kimi takes model and thinking level as session settings, not launch options.**
   `session/new` lists them (`configOptions`), and `session/set_config_option` switched the model
   and the next answer came from it.

## Decision

1. **Kimi runs over ACP under ADR-015**, through the shared driver, one supervised `kimi acp`
   program per task, with everything ADR-015 decides except §5's file-access rule, which this
   record replaces for Kimi.
2. **Plenipo offers Kimi file access, and answers every file request through Guard** (ADR-013,
   how Plenipo lets workers use your computer safely):
   - `initialize` offers `fs.readTextFile` and `fs.writeTextFile`, and no terminal.
   - `fs/read_text_file` and `fs/write_text_file` are file reads and writes under the worker's
     permissions: inside the project folder, not a blocked file, redacted, recorded, and with
     Ask-me approvals as for Plenipo's own tools. A worker with no folder, or no file permission,
     gets every file request refused.
   - The driver gains this as an option an ACP adapter can turn on; Grok keeps file access off.
3. **Plenipo answers Kimi's permission requests from Guard's decision:**
   - changing files follows the file permissions above;
   - **Kimi's own shell commands are always refused**, with a note to use Plenipo's
     `run_command`, so every program still runs through Plenipo's supervisor and command rules;
   - a call to Plenipo's tool server is allowed (Guard decides inside the call), as in ADR-015;
   - anything else is refused;
   - Plenipo never chooses "approve for this session" (`allow_always`): each approval covers
     one action, as in ADR-013.
4. **Kimi's mode is always `default`** (Kimi asks before acting). `auto` and `yolo` are never
   used; a worker with no permissions runs in `plan` (read-only) with every request refused.
5. **Model and thinking level are set with `session/set_config_option`** after `session/new` or
   `session/load` and before the prompt. The driver gains this as another adapter option. The
   model list stays declared in the adapter (ADR-014 §6), checked against `session/new`'s list.
6. **Sign-in and billing (ADR-007 §4, unchanged).** Before every task the adapter runs
   `kimi provider list` and continues only when the provider for the model is the Kimi
   subscription (`managed:kimi-code`, `source=oauth`). Plenipo runs only `kimi-code/*` models,
   passes no Kimi or Moonshot key variables, and ends a task that reports an authentication error
   as "sign-in required".

## Consequences

- Kimi is held more tightly than Claude Code and Codex. Its file reads also go through Guard,
  which closes, for Kimi, the gap Codex still has (ADR-013: Codex's own read-only commands can
  read outside the folder).
- The shared ACP driver gains two options (file access through Guard; session settings). Other
  ACP tools whose built-in tools cannot be switched off can use the first.
- Kimi still loads the owner's personal Kimi settings (for example installed skills and
  `AGENTS.md`). Their files are read through Plenipo, so the folder rules apply, but a worker may
  see skills the owner installed for their own use of Kimi.
- The Kimi adapter is built after the Grok branch (with ADR-015 and the shared driver) is merged.
- Each new Kimi version needs the owner's check again; the adapter records the version it was
  checked against (`checked_version()`).

## Alternatives considered

- **ADR-015 §5 as is (file access off).** Kimi's reads would not come to Plenipo, and may not
  ask, so a worker could read outside its folder.
- **Kimi's `plan` mode for every worker.** Read-only, but it leaves reads outside Guard and gives
  workers with file permissions no way to change files.
- **Letting Kimi run approved shell commands itself.** The command would run outside Plenipo's
  command rules and program log.
- **Prompt mode (`kimi -p`).** Fails two rules: the prompt must be an argument, and Kimi acts
  without asking.
