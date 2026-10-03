# Phase 25 — Acceptance Report (Wave 3)

|              |                                                                                                                                                                                                       |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 25 — Fixes and a simpler Plenipo before launch, **Wave 3** ("see and steer the work")                                                                                                                 |
| **Branch**   | `claude/relaxed-mccarthy-uxyguw` ([PR #156](https://github.com/Seckcey/plenipo/pull/156)), commits `f5a1280` to `a83c145`, the Windows fix `bf29ff2`, and the merge of `main`                         |
| **Verified** | `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm bindings` (no diff), and `pnpm docs:check`, here. GitHub CI on the pull request, Windows and the E2E included. |
| **Date**     | 2026-10-03 (Pacific time)                                                                                                                                                                             |
| **Result**   | All five items built, with two parts left for later (section 4). The owner's checks are next (section 3). Plenipo is made by 8 West Ventures, LLC.                                                    |

## In short

Wave 3 lets you see and steer the work. You can watch a worker type, see what its commands change,
stop one worker from its tile, stop all work with one red button (and allow it again), and ask a
busy manager or supervisor a question in a side chat. **What you need to do:** try the list in
section 3.

## 1. Items → result

| #   | Item (the checklist)                                 | Result   | Evidence                                                                                         |
| --- | ---------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------ |
| 3.1 | Watch the worker think and type, live                | **Done** | `45d4c02`: the Live conversation, plain-word steps, the progress line; **Open in Chat** (merge). |
| 3.2 | Watch shows changes made by commands too             | **Done** | `c941959`: "made by a command"; ADR-250.                                                         |
| 3.3 | A Stop button on every worker                        | **Done** | `f5a1280`; on the map between Chat and Watch (merge); the Windows file-name fix `bf29ff2`.       |
| 3.4 | Stop all: one red button on every page that shows it | **Done** | `f5a1280`: **Stop all work** and **Allow again**; ADR-199.                                       |
| 3.5 | Side chats with any manager or supervisor            | **Done** | `a83c145`: **Ask a question**, answer only, never touching the work; ADR-251.                    |

The tests for each item are named in the [checklist](phase-25-checklist.md).

## 2. Decisions

- [ADR-199 (Stop all work)](../adr/ADR-199-stop-all-work.md)
- [ADR-250 (Watch shows changes made by commands)](../adr/ADR-250-watch-shows-changes-made-by-commands.md)
- [ADR-251 (side chats)](../adr/ADR-251-side-chats.md)
- The merge of `main` brought ADR-200 (a live chat with each agent). The checklist says how 3.1
  and 3.5 relate to its Chat panel: both stay, for different jobs.

## 3. The owner's checks (Windows 11)

- You watch a worker type live.
- You stop one worker from its tile.
- **Stop all** from the canvas stops everything, and **Allow again** resumes.
- You ask a busy supervisor a question while it waits.

## 4. Not done yet

Claude Code's steps now show the moment they start (done after this report; see 3.1 in the
checklist).

- Codex's words as it types: they show as each message is complete (Codex's app-server, a stretch
  goal).
- A side chat continuing a copy of the AI tool's own conversation (`--fork-session` and the
  others'): every side chat starts fresh with a briefing, for now (ADR-251).

## 5. Fixed after the security review of #156

- **Stop all lasts across a restart.** Before, its hold on the AI work was kept only in memory, so
  work could start again after Plenipo restarted. Now the first organization's record turns the
  stop back on as Plenipo starts, until you press **Allow again**. If that record can't be read,
  the work waits too
  ([ADR-199 (Stop all work)](../adr/ADR-199-stop-all-work.md)).
