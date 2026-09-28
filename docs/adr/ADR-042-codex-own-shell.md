# ADR-042: Codex works through Plenipo's tools

- **Status:** Accepted (by the owner, 2026-09-28)
- **Date:** 2026-09-28
- **Phase:** 7 (follow-up)
- **Builds on:** [ADR-007 (how Plenipo runs Claude Code and Codex)](ADR-007-runtime-adapters.md),
  decision 5, and closes the gap
  [ADR-013 (Guard and the capability broker)](ADR-013-guard-capability-broker.md) left open
  ("Codex can still read outside the folder")

## Context

Plenipo runs a Codex worker with `codex exec --sandbox read-only`. That sandbox stops Codex's own
commands from writing files or using the network, but it lets them read any file the owner's
account can read, and those reads never pass through Plenipo: Guard does not decide them, the
blocked-files list does not apply, and the Ledger does not record them. The other AI tools are
held tighter: Claude Code runs with none of its own tools (`--tools ""`), Grok with a profile
that has none, and Kimi's file reads go through Plenipo (ADR-027). ADR-013 named the gap and said
closing it "needs Codex's shell turned off (a configuration key this phase does not assume)".

The Codex CLI Plenipo is checked against (codex-cli 0.157.1) has that key. Its `features` table
holds stable on/off switches, listed in OpenAI's configuration reference: `features.shell_tool`
("Enable the default shell tool for running commands; stable, on by default") and
`features.view_image` (the tool that reads a picture file from disk into the conversation). Both
are accepted on the command line as `-c features.shell_tool=false` and
`-c features.view_image=false`; `codex exec --strict-config` takes them as known settings and
rejects a misspelled one. Checked on 2026-09-28 by running that version against a stand-in for
OpenAI's service and reading the tool list Codex sent the model: by default it offered
`exec_command`, `write_stdin`, `view_image`, and `apply_patch` (with its goal, clock, and
teamwork tools); with the two settings, `exec_command`, `write_stdin`, and `view_image` were gone,
and its remaining script runner declares "no Node, no file system, no network access". Codex's
own code agrees: its command tools are added only while `shell_tool` is on, only a managed
company policy can pin the switch, and a Codex sub-agent's settings can only switch more off.

## Decision

1. **Every Codex turn switches Codex's own commands off.** `turn_args` adds
   `-c features.shell_tool=false -c features.view_image=false` right after
   `--skip-git-repo-check`, before the model, the effort, Plenipo's tool server, and `resume`.
   The read-only sandbox stays as a second wall; it still refuses the writes of Codex's
   `apply_patch`, which Plenipo records as a file change either way.
2. **Codex reads files only through Plenipo's tools.** A worker with permissions gets
   `read_file`, `search_files`, `run_command`, and the git tools from Plenipo's tool server,
   where Guard decides, the blocked-files list applies, secrets are hidden, and every use is
   recorded. A worker without permissions can only answer.
3. **The posture says so.** The AI tools page reads: "Codex's own commands are off: it cannot
   run commands or read files on its own, and its read-only sandbox allows no writes and no
   network. A worker with permissions gets Plenipo's file, program, and git tools, each checked
   by Plenipo Guard."
4. **A contract test holds it.** `codex_own_commands_are_switched_off` checks both settings in a
   new and in a resumed turn, their place before the model and the session, and the posture's
   words.

## Consequences

- The first promise in SECURITY.md ("a worker stays inside its project's folder") now holds for
  Codex workers as it does for the others. A Codex worker's every file read is a Guard decision
  in the trail.
- A Codex worker that used its own shell for quick looks (listing a folder, reading a file) now
  asks Plenipo's tools for them; a worker without file permissions is told it has none. The task
  itself is unchanged.
- Plenipo relies on Codex honoring its own documented settings. Plenipo does not pass
  `--strict-config`, so a later Codex that renamed the keys would only warn in its own log, and
  the shell would be back. The adapter records the version it was checked against (0.157.1),
  and the check of each new Codex version must repeat the tool-list check above.
- Codex's Windows sandbox is newer than its Linux one, and Plenipo cannot try it from Linux.
  The owner's check that `codex exec --sandbox read-only` refuses a write and a web fetch on
  Windows stands; with the shell off, the sandbox guards only `apply_patch`.

## Alternatives considered

- **Tell the truth in the posture and allow Codex only for roles that may read files.** It keeps
  Codex's reads unseen by Guard and the trail; with the switch at hand there is no reason to.
- **Codex's app-server with approval callbacks** (the revisit note in ADR-007 and ADR-013).
  Richer, but a long-lived protocol that would replace the working `exec` path; the two settings
  close the gap in one line.
- **`--strict-config`,** so a renamed key would stop the turn instead of quietly bringing the
  shell back. It also rejects any field in the owner's own Codex settings file that this Codex
  version does not know, which would stop Codex workers after an owner's Codex update. Left out;
  the version check carries the duty.
- **Also switching off `apply_patch`.** It is Codex's file-writing tool; the read-only sandbox
  already refuses its writes and Plenipo records each attempt as a file change, so this record
  does not need it.
