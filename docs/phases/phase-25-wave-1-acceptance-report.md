# Phase 25 — Acceptance Report (Wave 1)

|              |                                                                                                                                                                                                       |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 25 — Fixes and a simpler Plenipo before launch, **Wave 1** ("fix what's broken")                                                                                                                      |
| **Branch**   | `claude/relaxed-mccarthy-uxyguw` ([PR #156](https://github.com/Seckcey/plenipo/pull/156)), commit `c18fe82`, with the end-to-end test fix `feed202`                                                   |
| **Verified** | `pnpm check`, `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm bindings` (no diff), and `pnpm docs:check`, here. GitHub CI on the pull request, Windows and the E2E included. |
| **Date**     | 2026-10-03 (Pacific time)                                                                                                                                                                             |
| **Result**   | All eight items built. The owner's checks on Windows 11 are next (section 3). Plenipo is made by 8 West Ventures, LLC.                                                                                |

## In short

Wave 1 fixed the eight things that were broken in your bug reports. A new organization no longer
freezes Plenipo, Usage shows numbers, a card is green when it works, a key saved once works
everywhere, and Watch shows a supervisor's team. **What you need to do:** try the list in section 3
on your Windows 11 PC.

## 1. Items → result

| #   | Item (the checklist)                                  | Result   | Evidence                                                                                     |
| --- | ----------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------- |
| 1.1 | Making a new organization froze Plenipo               | **Done** | The window commands run off the window's thread; a test keeps every org command async.       |
| 1.2 | Usage numbers missing for Claude Code and Codex       | **Done** | Usage adds up every organization; every organization asks for the plan left; **Check plan**. |
| 1.3 | One light per AI company                              | **Done** | Green when the subscription or the key works, and it says which.                             |
| 1.4 | Paid keys work in every organization                  | **Done** | One set of keys for the PC, kept with the first organization; ADR-192.                       |
| 1.5 | Subscriptions first in every AI tool picker           | **Done** | Pickers list subscriptions first, paid tools once their key works.                           |
| 1.6 | Copilot as code reviewer ("who made it is not known") | **Done** | A model picked by name is kept, with a warning naming what can't be ruled out; ADR-191.      |
| 1.7 | "What's stuck" opens the stuck thing                  | **Done** | The Stuck and Objectives going tiles open what they count.                                   |
| 1.8 | Watch on every tile, and the team's work              | **Done** | Watch shows a lead's team, says why it is empty, and is on every active tile; ADR-193.       |

The tests for each item are named in the [checklist](phase-25-checklist.md).

## 2. Decisions

- [ADR-191 (a model you pick by name is your choice)](../adr/ADR-191-a-model-you-pick-by-name-is-your-choice.md)
- [ADR-192 (one set of paid AI keys for the whole PC)](../adr/ADR-192-one-set-of-paid-keys-for-the-pc.md)
- [ADR-193 (Watch shows the team's work)](../adr/ADR-193-watch-shows-the-team.md)

## 3. The owner's checks (Windows 11)

- A new organization opens without freezing (five times in a row).
- Claude Code's and Codex's usage show.
- A saved key or a signed-in subscription turns the light green.
- Grok's subscription is used for a manager.
- Copilot can be the code reviewer.
- "What's stuck" opens the stuck thing.
- Watch on a supervisor shows its team's changes.

## 4. Moved to later items

- A background plan check telling the open page: done in 4.3.
- A subscription card showing its key's usage: done with 2.1.
- The full fix for a subscription with its company's key as a backup: done in 4.4.
