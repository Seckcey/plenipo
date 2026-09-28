# ADR-055: Watch — seeing a worker write code as it happens

- **Status:** Accepted (by the owner, 2026-09-28, as recommended; §13, a refused edit's record
  keeping no text, after an explanation)
- **Date:** 2026-09-28
- **Phase:** 18
- **Carries out:** ADR-039 (the owner's notes) §2.12, "watch a worker write code, live"
- **Amends:** ADR-031 (the terminal panel) — the bottom panel gains Watch tabs for code, beside
  your terminals and the server watch tabs

> **On screen** (ADR-010, plain words and rank names): **Watch** (the button and the tab,
> "Watch · Senior Developer"), **being written — not saved yet**, **waiting for your approval**,
> **saved**, **refused**, **not saved**, **new** and **changed** lines, **Follow along**, **Pin
> this file**, and **Stop**. This record keeps the code's words (tool call, diff, stream).

## In short

Press **Watch** on a working agent, and a tab opens at the bottom of the window showing the file
it is changing, with new and changed lines marked as each change lands, and a list of every file
it has touched in this objective. With Claude Code — and Kimi, whose real program was recorded
doing the same — you see the code appear as the AI writes it, marked **being written — not saved
yet**, then **saved**, or **refused** if Guard said no. The tab only shows what Guard already
lets that worker touch: never a blocked file, never a secret, never a file outside its working
copy. It is read-only; **Stop** stops the worker, as elsewhere. Accepting this record means
building it as written below.

## Context

Phase 18 of `ROLLOUT_PLAN.md` asks for a **Watch** tab "in the bottom panel, beside the terminals
and the server watch tabs (ADR-031), and a Watch button on any working agent on the canvas and in
its properties panel"; "the file the worker is changing, with new and changed lines highlighted
as each change lands, and a list of every file it has touched in this objective (click one to see
its changes)"; streaming AI tools shown "as it is written, marked being written — not saved yet,
then saved, or refused if Guard refused it"; "follow along automatically, or pin one file";
"Stop stops the worker, as elsewhere; nothing typed in the tab reaches the worker". Its technical
notes: Watch "reads the file changes Plenipo already carries out for workers: its own
`write_file` and `edit_file` tools (Claude Code, Codex, Grok) and ACP's `fs/write_text_file`
(Kimi, ADR-027) … No new permission: the tab shows only what Plenipo already sees"; "each other
AI tool is checked on its real program in this phase"; "the Ledger records each saved change, as
it records tool calls now. The letter-by-letter preview is shown, not stored. Large files and
binary files show a summary"; "the Watch tab never writes to a working copy (ADR-016)".

The owner's rules for this phase add: "Watching a worker write code shows only what Guard
already allows that worker to touch. Secrets and blocked files are never shown, and nothing is
written to disk that Guard would refuse", and "logs and diagnostics files never hold secrets".

What the code does today (v1.10.0, read at `c5d1a71`):

- **Every change goes through one place.** Plenipo's `write_file` and `edit_file` (Claude Code,
  Codex, Grok) and Kimi's `fs/write_text_file` all reach the broker's `act()`
  (`crates/capabilities/src/broker.rs`), which checks the path (inside the working copy, not a
  blocked file, not git's own folder), asks Guard, asks you when the permission says Ask me, and
  then writes. `edit_file` already reads the whole file and makes the new one; `write_file` does
  not read the old file. Neither returns the before and after.
- **What is kept:** each call is recorded (`capability.used`) with a short summary (the file and
  its size); a refusal is recorded (`guard.denied`) with the reason. For an edit, today's summary
  keeps up to 200 characters of the old and new text, even when Guard refused it.
- **Streaming:** Claude Code is already run with `--include-partial-messages`, which streams each
  tool call while the model writes it (`content_block_start` and `input_json_delta` lines);
  Plenipo reads only the text parts. Kimi's recorded run (0.34.0,
  `crates/runtime/tests/fixtures/kimi-0.34.0/acp/`) streams its Write tool's arguments the same
  way, in `tool_call_update` notes marked `in_progress`; Plenipo ignores them. Codex (`exec
--json`) sends each tool call whole; Grok's streaming of tool calls over ACP has no recording;
  Ollama's models do not use tools.
- **The bottom panel** (ADR-031) holds your terminals and read-only server watch tabs, built from
  Ledger events, with Stop and Disconnect. Stopping a worker elsewhere is **Cancel task**
  (`cancel_agent_turn`).

## Decision

### Where

1. **Watch** is a button on any working agent: on the canvas (beside the tile of an agent that is
   working) and in its details (Overview tab). The bottom panel's **New** menu also lists
   "Watch a worker" with the agents working now.
2. **One Watch tab per agent you watch,** named "Watch · Senior Developer", beside your terminals
   and the server watch tabs. It stays, and readable, after the worker finishes, until you close
   it. Watching an on-call agent shows the changes of each worker it brings in, labeled by task.

### What it shows

3. **The files touched in this objective,** in a list: each file's name, whether it was created
   or changed, lines added and removed, and its state. Click one to see it.
4. **The file:** the file as it is after the change, with **new** lines and **changed** lines
   marked (a colored bar **and** a word, never color alone), and "3 lines removed" where lines
   went. It scrolls to the change.
5. **Follow along** (on by default) shows the file changed most recently; **Pin this file** keeps
   one file in view.
6. **States,** each with a mark and a word:
   - **being written — not saved yet:** the AI is writing it now (streaming AI tools);
   - **waiting for your approval:** the change needs your Approve (the permission says Ask me);
   - **saved:** Plenipo wrote it;
   - **refused:** Guard or you said no, with the reason — the file name and why, never its
     content;
   - **not saved:** the AI tool stopped before sending it, or the change could not be made
     ("the text to replace was not found").
7. **Large and non-text files show a summary,** not their contents: over 256 KB or 5,000 lines,
   "Large file: 1.2 MB · 14 lines added, 3 removed"; not text, "Not a text file (34 KB)".

### Where the changes come from

8. **Saved changes come from the broker, as each is applied.** After Guard allows a change and
   Plenipo writes it, the broker publishes it: the worker, its task, the file's path inside the
   working copy, and the lines before and after (the old file is read just before `write_file`
   replaces it). Refusals come from the same place, at the moment Guard or you refuse.
9. **Letter by letter comes from the AI tool's own stream:**
   - **Claude Code:** its `input_json_delta` lines for Plenipo's `write_file` and `edit_file`;
   - **Kimi:** its `tool_call_update` notes marked `in_progress` for its Write and Edit tools
     (as recorded from its real program, 0.34.0);
   - **Grok:** the same ACP notes, if its real program sends them (not recorded yet);
   - **Codex:** each change when saved (its `exec --json` sends a tool call whole); **Ollama:** no
     file tools.
     Each is checked again on its real program on your PC (the acceptance walk-through), and the
     acceptance report says what each did.
10. **The preview becomes the saved change.** Plenipo matches the change being written with the
    one the broker then applies (same conversation, same file, in order): it turns **saved**, or
    **refused**. A preview that never arrives turns **not saved** when its task ends.

### Only what Guard allows

11. **A preview shows nothing until its file is known and checked.** The file's name usually
    comes first; any text that comes before it waits. Then Plenipo checks the file the way Guard
    will for that worker's current step: inside its working copy, not a blocked file, not inside
    git's own folder, and its file permission is Allowed or Ask me. If not, the tab shows "a
    change Plenipo will not show" and never its text.
12. **Secrets are hidden** in previews and saved changes by the same filter as everywhere else
    (your stored secrets, and anything that looks like a key or password), applied to the whole
    text each time, not piece by piece, so a secret split across two pieces is still caught.
13. **Nothing refused reaches the disk:** previews are never written anywhere; a refused change
    keeps only its file name, its size, and why. Today's refusal record for an edit keeps up to
    200 characters of the text Guard refused; from this phase it keeps only the file and the size.
14. **Only the main window hears Watch.** Its live updates go to Plenipo's main window alone
    (`plenipo://watch`), never to the sign window or a web page.

### What is kept

15. **The Ledger records each saved change, as it records tool calls:** the call's record
    (`capability.used`) gains the file, whether it was created or changed, and how many lines
    were added and removed. No file contents go into the Ledger (ADR-006 §9: the Ledger keeps
    records, not files), the logs, or the diagnostics file.
16. **The lines themselves are held while Plenipo runs,** for each worker's objective, up to 64 MB
    in all (the oldest go first, then show as a summary). After a restart, the list of files
    comes back from the Ledger with its counts; a change from before the restart says "The lines
    are shown only while Plenipo runs; the file is in the working copy."
17. **The letter-by-letter preview is shown, not stored.**

### Read-only

18. **The tab has no place to type,** and its commands only read (`get_watch`: the files and
    states for a worker; `get_watch_change`: one change's file). None of them writes to a working
    copy (ADR-016: one writer per working copy). Both are the main window's alone, refused from
    the sign window and from any web page, with IPC tests.
19. **Stop** stops the worker's task, the same as **Cancel task** on the Workers page.

### How lines are compared

20. **A line-by-line comparison** from `similar`, a widely used Rust library (MIT or Apache 2.0),
    marks new, changed, and removed lines.

## Your choices (recommended first)

- **One Watch tab per agent.** _Or:_ a single Watch tab with a picker — tidier, but two workers
  writing at once would fight over one view.
- **Show changes that wait for your approval** ("waiting for your approval"), since you are the
  one deciding. _Or:_ show nothing until Guard has allowed the change.
- **Keep the lines in memory while Plenipo runs, and only the record in the Ledger.** _Or:_ keep
  the changed lines in the Ledger too (up to 64 KB a change), so they survive a restart — but the
  Ledger would grow with your code, which the working copy and its branch already hold.

## Consequences

- You can watch code being written without giving any worker a new permission: the tab shows
  what Plenipo already carries out.
- `write_file` now reads the file it replaces, to show what changed. It reads only a file the
  worker may already write, inside its working copy.
- Claude Code's and Kimi's streams carry more lines Plenipo now reads; each is checked before
  anything is shown, and nothing from them is stored.
- One new library (`similar`).

## Alternatives considered

- **Watch the worker's screen or the AI tool's own display.** Rejected (ADR-039): Plenipo already
  has every change, in order, with its file name.
- **Show a preview before its file is known.** Rejected: the text could belong to a blocked file.
- **Read the working copy on disk to show changes.** Rejected: Plenipo already has the change in
  hand when it writes it, and reading the disk again could show a file Guard never let the worker
  touch.
- **Let the owner type in the tab.** Rejected: one writer per working copy (ADR-016); editing
  project files is Phase 21, with its own rules.
