# ADR-250: Watch shows changes made by commands too

- **Status:** Accepted (the owner, 2026-10-03: "The watch doesn't seem to let the user watch the
  agent code."; item 3.2 of [ADR-190 (Phase 25 starts)](ADR-190-phase-25-starts.md), accepted the
  same day).
- **Date:** 2026-10-03
- **Phase:** 25, Wave 3 (item 3.2)
- **Number:** the first after Phase 25's block (ADR-190 to ADR-199, all used). First written as
  ADR-200; renumbered on 2026-10-03, when another session put ADR-200 to ADR-202 on `main`. Phase
  25 continues in the 250s.
- **Amends:** [ADR-055 (Watch a worker write code)](ADR-055-watch-a-worker-write-code.md): Watch
  showed only the files a worker wrote with Plenipo's own file tools.

> **On screen** (ADR-010, plain words and rank names): a file in Watch's list says "made by a
> command" beside "created" or "changed". The empty Watch tab says it shows "the files they write,
> and the files the commands they run make or change".

## In short

Watch saw only files written through Plenipo's file tools. A file a command made (`npm create`, a
formatter, `sed`, a git step) never showed, so a worker could change a lot while Watch said
nothing.

**Accepting this record means:**

1. **Before each command or git step** a worker runs in a working copy or project folder, Plenipo
   notes its files: each file's size and time, and the text of files up to 256 KB (16 MB in all).
2. **After it**, Plenipo compares. Each file made or changed shows in Watch as **made by a
   command**, with its lines marked when its text before was noted (a large file shows its size).
   A file whose text did not change (only its time) is not shown. At most 50 files per command.
3. **Private files are never read**: Guard's private-files list is left out before and after, the
   same as for Plenipo's file tools. Secrets are hidden in what Watch shows, the same way.
4. **Left out:** the folders a command fills by itself (`.git`, `node_modules`, `target`, build
   and cache folders), and a working copy of more than 20,000 files (it would take too long).
5. **The record** of the command (`capability.used`) lists the files it made or changed with their
   line counts, never their text, like a saved file change (ADR-055).

## Decision

- The comparison runs off Plenipo's busy threads, around the command, in the broker that runs it
  (`crates/capabilities/src/command_changes.rs`).
- After Plenipo starts again, the files a command changed are not listed again from the record
  (lines are kept only while Plenipo runs, as for every change).

## Consequences

- Watch shows the real work of a worker that runs a generator, a formatter, or git.
- A command takes a little longer in a large working copy: noting its files reads up to 16 MB.

## Alternatives considered

- **Ask git what changed.** Rejected: not every project folder is a git repository, and git
  doesn't see files it ignores.
- **Watch the folder for changes as they happen.** Rejected for now: it would also show changes
  the owner or another program makes, and can't say which command made them.
